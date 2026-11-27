use super::*;

#[test]
fn test_conversion_char_refines_reading() {
    let mut engine = InputMethodEngine::new();

    // Type "あい" and enter conversion
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    // Typing during conversion must NOT commit — it drops back to the
    // composition and extends the reading (incremental-search feel).
    let result = engine.process_key(&press('k'));
    assert!(result.consumed);
    assert!(
        !result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(_))),
        "typing must refine, not commit"
    );
    assert!(matches!(engine.state(), InputState::Composing { .. }));

    engine.process_key(&press('a'));
    assert_eq!(engine.input_buf.reading(), "あいか");

    // The refined reading converts and commits as one unit.
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert!(
        result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(_)))
    );
}

#[test]
fn test_alphabet_mode_space_inserts_literal_space() {
    let mut engine = InputMethodEngine::new();

    // Enter alphabet mode via Shift+N
    engine.process_key(&press_shift('N'));
    assert!(engine.mode.current() == InputMode::Alphabet);

    // Type "ew"
    engine.process_key(&press('e'));
    engine.process_key(&press('w'));
    assert_eq!(engine.preedit().unwrap().text(), "New");

    // Space → should insert literal space, NOT start conversion
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "New ");

    // Type "york"
    engine.process_key(&press('y'));
    engine.process_key(&press('o'));
    engine.process_key(&press('r'));
    engine.process_key(&press('k'));
    assert_eq!(engine.preedit().unwrap().text(), "New york");
}

#[test]
fn test_stray_keys_are_consumed_during_conversion() {
    // Unbound chords and special keys must be consumed as no-ops while the
    // conversion window is shown — leaking them would let the application
    // act on them (e.g. Ctrl+R reloading a browser page) mid-conversion.
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    let cursor = engine.candidates().unwrap().cursor();

    for key in [
        press_ctrl(Keysym(0x0067)), // Ctrl+g (unbound)
        press_ctrl(Keysym(0x0077)), // Ctrl+w (unbound; closes a browser tab)
        press_key(Keysym(0xffc2)),  // F5
    ] {
        let result = engine.process_key(&key);
        assert!(result.consumed, "key must not leak to the application");
        assert!(matches!(engine.state(), InputState::Conversion { .. }));
        assert_eq!(engine.candidates().unwrap().cursor(), cursor);
    }
}

/// Text of the first Commit action in a result, if any.
fn committed(result: &EngineResult) -> Option<String> {
    result.actions.iter().find_map(|a| match a {
        EngineAction::Commit(text) => Some(text.clone()),
        _ => None,
    })
}

#[test]
fn test_bare_digit_during_conversion_refines_instead_of_selecting() {
    // Digits are plain text input everywhere: during conversion they extend
    // the reading like any printable char, never select a candidate.
    let mut engine = InputMethodEngine::new();
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"あい","candidates":[{"surface":"藍","score":1.0}]}]"#,
    ));

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let result = engine.process_key(&press('2'));
    assert!(committed(&result).is_none(), "a digit must not commit");
    assert_eq!(engine.input_buf.reading(), "あい2");
}

#[test]
fn test_ctrl_digit_selects_candidate_during_conversion() {
    let mut engine = InputMethodEngine::new();
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"あい","candidates":[
            {"surface":"藍","score":2.0},
            {"surface":"愛","score":1.0}
        ]}]"#,
    ));

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    let shown: Vec<String> = engine
        .candidates()
        .unwrap()
        .candidates()
        .iter()
        .map(|c| c.text.clone())
        .collect();

    let result = engine.process_key(&press_ctrl(Keysym::KEY_2));
    assert_eq!(committed(&result).as_deref(), Some(shown[1].as_str()));
    assert!(matches!(engine.state(), InputState::Empty));
    assert!(engine.input_buf.is_empty(), "buffer must be cleared");
}

#[test]
fn test_ctrl_digit_selects_candidate_while_composing() {
    // The suggestion window is on screen while composing, so Ctrl+digit
    // commits straight from it — no Space needed first.
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"あい","candidates":[{"surface":"藍","score":1.0}]}]"#,
    ));

    engine.process_key(&press('a'));
    let result = engine.process_key(&press('i'));
    let shown = result
        .actions
        .iter()
        .find_map(|a| match a {
            EngineAction::ShowCandidates(list) => Some(list.candidates().to_vec()),
            _ => None,
        })
        .expect("suggestion window");
    // The dictionary entry's position depends on what else the suggestion
    // list holds (a loaded model contributes its own row), so select it by
    // the digit it is actually shown under.
    let digit = shown
        .iter()
        .position(|c| c.text == "藍")
        .expect("dictionary candidate in the suggestion window")
        + 1;
    assert!(matches!(engine.state(), InputState::Composing { .. }));

    let result = engine.process_key(&press_ctrl(Keysym(b'0' as u32 + digit as u32)));
    assert_eq!(committed(&result).as_deref(), Some("藍"));
    assert!(matches!(engine.state(), InputState::Empty));
}

#[test]
fn test_ctrl_digit_with_no_suggestion_is_consumed() {
    // Nothing to select: the chord must still be swallowed rather than
    // leaking to the application mid-composition.
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;

    engine.process_key(&press('a'));
    let result = engine.process_key(&press_ctrl(Keysym::KEY_9));
    assert!(result.consumed);
    assert!(committed(&result).is_none());
    assert!(matches!(engine.state(), InputState::Composing { .. }));
}

#[test]
fn test_emoji_digit_selection_does_not_pollute_learning() {
    // Committing an emoji by number must not record `:query` → 😀 into the
    // kana-keyed learning cache, and must leave emoji mode.
    let mut engine = engine_with_learned("あい", "愛");
    engine.process_key(&press(':'));
    assert_eq!(engine.mode.current(), InputMode::Emoji);
    for ch in "smile".chars() {
        engine.process_key(&press(ch));
    }

    let result = engine.process_key(&press_ctrl(Keysym::KEY_1));
    assert!(committed(&result).is_some(), "emoji must commit");
    assert_eq!(engine.mode.current(), InputMode::Hiragana);
    let learned = engine.learning.as_ref().unwrap();
    assert!(
        learned.lookup(":smile").is_empty(),
        "emoji query must not enter the learning cache"
    );
}

#[test]
fn test_arrow_in_conversion_returns_to_composing_and_moves_caret() {
    // Matching live conversion: a caret key dissolves the conversion and
    // moves the caret in the raw composition.
    let mut engine = InputMethodEngine::new();
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    assert_eq!(engine.input_buf.cursor(), 3); // き ょ う
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let result = engine.process_key(&press_key(Keysym::LEFT));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.input_buf.cursor(), 2, "caret must move left");

    engine.process_key(&press_key(Keysym::END));
    assert_eq!(engine.input_buf.cursor(), 3);
}

#[test]
fn test_arrow_in_source_view_dissolves_the_filter() {
    // From the Ctrl+I model view: the caret key exits to editing and the
    // filter dies with the conversion state.
    let mut engine = InputMethodEngine::new();
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_ctrl(Keysym::KEY_I));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let result = engine.process_key(&press_key(Keysym::LEFT));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.input_buf.cursor(), 2);
    assert!(engine.state().filter().is_none());
}

#[test]
fn test_ctrl_b_in_conversion_moves_caret_like_left() {
    let mut engine = InputMethodEngine::new();
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let result = engine.process_key(&press_ctrl(Keysym::KEY_B));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.input_buf.cursor(), 2);
}

#[test]
fn test_shift_space_steps_back_a_candidate() {
    // Shift+Space is prev-candidate, the mirror of Space, like Shift+Tab.
    let mut engine = InputMethodEngine::new();
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"あい","candidates":[
            {"surface":"藍","score":2.0},
            {"surface":"愛","score":1.0}
        ]}]"#,
    ));
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    let before = engine.candidates().unwrap().cursor();
    engine.process_key(&press_key(Keysym::SPACE));
    assert_eq!(engine.candidates().unwrap().cursor(), before + 1);

    let result = engine.process_key(&press_shift_key(Keysym::SPACE));
    assert!(result.consumed);
    assert_eq!(engine.candidates().unwrap().cursor(), before);
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    assert!(committed(&result).is_none(), "Shift+Space must not commit");
}

fn show_candidates_action(result: &EngineResult) -> &CandidateList {
    result
        .actions
        .iter()
        .rev()
        .find_map(|a| match a {
            EngineAction::ShowCandidates(list) => Some(list),
            _ => None,
        })
        .expect("ShowCandidates action")
}

#[test]
fn test_up_from_top_opens_upper_kana_panel_and_down_closes() {
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"きょう","candidates":[{"surface":"今日","score":1.0},{"surface":"京","score":2.0}]}]"#,
    ));

    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    assert_eq!(engine.candidates().unwrap().selected_text(), Some("今日"));
    assert_eq!(engine.candidates().unwrap().active_page_cursor(), Some(0));

    // 1st Up: opens upper panel with ひらがな (closest to typing line) selected
    let result = engine.process_key(&press_key(Keysym::UP));
    let shown = show_candidates_action(&result);
    assert_eq!(shown.active_page_cursor(), None);
    let upper = shown.upper().expect("upper panel should open");
    assert_eq!(upper.candidates[0].text, "キョウ");
    assert_eq!(upper.candidates[1].text, "きょう");
    assert_eq!(upper.cursor, 1);
    assert_eq!(engine.preedit().unwrap().text(), "きょう");

    // 2nd Up: moves to カタカナ
    let result = engine.process_key(&press_key(Keysym::UP));
    let shown = show_candidates_action(&result);
    assert_eq!(shown.upper().unwrap().cursor, 0);
    assert_eq!(engine.preedit().unwrap().text(), "キョウ");

    // 3rd Up: clamps at カタカナ (no wrap-around)
    let result = engine.process_key(&press_key(Keysym::UP));
    let shown = show_candidates_action(&result);
    assert_eq!(shown.upper().unwrap().cursor, 0);
    assert_eq!(engine.preedit().unwrap().text(), "キョウ");

    // Down: moves back to ひらがな
    let result = engine.process_key(&press_key(Keysym::DOWN));
    let shown = show_candidates_action(&result);
    assert_eq!(shown.upper().unwrap().cursor, 1);
    assert_eq!(engine.preedit().unwrap().text(), "きょう");

    // Space (or Down) from ひらがな closes upper panel and restores normal candidate 0
    let result = engine.process_key(&press_key(Keysym::SPACE));
    let shown = show_candidates_action(&result);
    assert!(shown.upper().is_none());
    assert_eq!(shown.active_page_cursor(), Some(0));
    assert_eq!(engine.preedit().unwrap().text(), "今日");
}

#[test]
fn test_enter_commits_upper_kana_and_records_learning() {
    let mut engine = engine_with_learned("きょう", "京");
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_key(Keysym::UP)); // ひらがな
    engine.process_key(&press_key(Keysym::UP)); // カタカナ

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert_eq!(committed(&result).as_deref(), Some("キョウ"));
    assert!(matches!(engine.state(), InputState::Empty));
    assert!(
        engine
            .learning
            .as_ref()
            .unwrap()
            .lookup("きょう")
            .iter()
            .any(|(surface, _)| surface == "キョウ"),
        "committed upper katakana must be recorded under reading"
    );
}

#[test]
fn test_ctrl_digit_commits_normal_candidate_while_upper_open() {
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"きょう","candidates":[{"surface":"今日","score":1.0},{"surface":"京","score":2.0}]}]"#,
    ));
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_key(Keysym::UP));
    assert!(engine.candidates().unwrap().has_upper_focus());

    let result = engine.process_key(&press_ctrl(Keysym::KEY_2));
    assert_eq!(committed(&result).as_deref(), Some("京"));
    assert!(matches!(engine.state(), InputState::Empty));
}

#[test]
fn test_shift_space_and_ctrl_p_wrap_without_opening_upper() {
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"きょう","candidates":[{"surface":"今日","score":2.0},{"surface":"京","score":1.0}]}]"#,
    ));
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    let last_idx = engine.candidates().unwrap().len() - 1;

    engine.process_key(&press_shift_key(Keysym::SPACE));
    assert!(!engine.candidates().unwrap().has_upper_focus());
    assert_eq!(engine.candidates().unwrap().cursor(), last_idx);

    // Return to 0, then test Ctrl+P
    engine.process_key(&press_key(Keysym::SPACE));
    assert_eq!(engine.candidates().unwrap().cursor(), 0);
    engine.process_key(&press_ctrl(Keysym::KEY_P));
    assert!(!engine.candidates().unwrap().has_upper_focus());
    assert_eq!(engine.candidates().unwrap().cursor(), last_idx);
}

#[test]
fn test_up_from_nonzero_cursor_steps_up_normally() {
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;
    engine.dicts.user = Some(dict_from_json(
        r#"[{"reading":"きょう","candidates":[{"surface":"今日","score":2.0},{"surface":"京","score":1.0}]}]"#,
    ));
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_key(Keysym::DOWN));
    assert_eq!(engine.candidates().unwrap().cursor(), 1);

    engine.process_key(&press_key(Keysym::UP));
    assert!(!engine.candidates().unwrap().has_upper_focus());
    assert_eq!(engine.candidates().unwrap().cursor(), 0);
}

#[test]
fn test_up_does_not_open_upper_in_filter_emoji_or_non_kana() {
    // 1. Source-filtered view (Ctrl+I)
    let mut engine = InputMethodEngine::new();
    for ch in "kyou".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_ctrl(Keysym::KEY_I));
    engine.process_key(&press_key(Keysym::UP));
    assert!(!engine.candidates().unwrap().has_upper_focus());

    // 2. Non-kana reading ("123")
    let mut engine = InputMethodEngine::new();
    for ch in "123".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_key(Keysym::UP));
    assert!(!engine.candidates().unwrap().has_upper_focus());

    // 3. Emoji mode (":smile")
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press(':'));
    for ch in "smile".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_key(Keysym::UP));
    assert!(!engine.candidates().unwrap().has_upper_focus());
}

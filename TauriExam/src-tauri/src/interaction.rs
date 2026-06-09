//! 交互题（选择 / Hotspot / 拖拽）的结构化模型构建与判分数据查询。

use rusqlite::{params, Connection};

use crate::banks::get_question;
use crate::models::{InteractionModel, InteractionOption, InteractionRow, InteractionSlot};
use crate::storage::{open_bank, table_exists};

#[tauri::command]
pub(crate) fn get_interaction_model(bank_id: String, question_id: String) -> Result<InteractionModel, String> {
    let conn = open_bank(&bank_id)?;
    let question = get_question(bank_id, question_id)?;
    let normalized = question.question_type.to_lowercase();
    let interaction_kind = question_interaction_kind(&question.question_md);

    if !question.options.is_empty() {
        return Ok(InteractionModel {
            kind: if normalized.contains("multiple") { "multiple_choice" } else { "single_choice" }.to_string(),
            can_auto_grade: true,
            message: "标准选择题，使用 options 表自动判分。".to_string(),
            options: question
                .options
                .into_iter()
                .map(|option| InteractionOption {
                    key: option.option_key,
                    text: option.option_text,
                    group: None,
                    is_distractor: false,
                    sort_order: option.sort_order,
                })
                .collect(),
            rows: Vec::new(),
            slots: Vec::new(),
            answer_key: Vec::new(),
        });
    }

    if table_exists(&conn, "interaction_targets") && table_exists(&conn, "interaction_options") {
        let options = query_interaction_options(&conn, "interaction_options", &question.id)?;
        let targets = query_unified_interaction_targets(&conn, &question.id)?;
        if !targets.is_empty() && !options.is_empty() {
            if normalized.contains("hotspot") || interaction_kind.as_deref() == Some("dropdown_hotspot") {
                return Ok(InteractionModel {
                    kind: "hotspot".to_string(),
                    can_auto_grade: true,
                    message: "检测到统一 interaction_options / interaction_targets，已按 Hotspot 下拉题渲染。".to_string(),
                    options,
                    rows: targets
                        .into_iter()
                        .map(|target| InteractionRow {
                            id: target.id,
                            prompt: target.label,
                            option_group: target.option_group,
                            correct_selection: target.correct_option,
                            sort_order: target.sort_order,
                        })
                        .collect(),
                    slots: Vec::new(),
                    answer_key: Vec::new(),
                });
            }
            if normalized.contains("drag") || normalized.contains("drop") || matches!(interaction_kind.as_deref(), Some("drag_drop" | "ordered_list")) {
                let is_ordered = interaction_kind.as_deref() == Some("ordered_list");
                let slots: Vec<InteractionSlot> = targets
                    .into_iter()
                    .map(|target| InteractionSlot {
                        id: target.id,
                        label: if is_ordered { format!("{}. {}", target.sort_order, target.label) } else { target.label },
                        correct_option: target.correct_option,
                        sort_order: target.sort_order,
                    })
                    .collect();
                return Ok(InteractionModel {
                    kind: "drag_drop".to_string(),
                    can_auto_grade: true,
                    message: if is_ordered {
                        "检测到统一 interaction_options / interaction_targets，已按有序拖拽题渲染。".to_string()
                    } else {
                        "检测到统一 interaction_options / interaction_targets，已按 Drag Drop 题渲染。".to_string()
                    },
                    options,
                    answer_key: slots.iter().filter_map(|slot| slot.correct_option.clone()).collect(),
                    rows: Vec::new(),
                    slots,
                });
            }
        }
    }

    if normalized.contains("hotspot") && table_exists(&conn, "hotspot_rows") {
        let rows = query_hotspot_rows(&conn, &question.id)?;
        if !rows.is_empty() {
            return Ok(InteractionModel {
                kind: "hotspot".to_string(),
                can_auto_grade: true,
                message: "检测到当前题的 hotspot_rows，已启用结构化 Hotspot 框架。".to_string(),
                options: query_interaction_options(&conn, "hotspot_options", &question.id)?,
                rows,
                slots: Vec::new(),
                answer_key: Vec::new(),
            });
        }
    }

    if normalized.contains("drag") && table_exists(&conn, "drag_slots") {
        let slots = query_drag_slots(&conn, &question.id)?;
        if !slots.is_empty() {
            return Ok(InteractionModel {
                kind: "drag_drop".to_string(),
                can_auto_grade: true,
                message: "检测到当前题的 drag_slots，已启用结构化 Drag Drop 自动判分。".to_string(),
                options: query_interaction_options(&conn, "drag_options", &question.id)?,
                answer_key: slots.iter().filter_map(|slot| slot.correct_option.clone()).collect(),
                rows: Vec::new(),
                slots,
            });
        }
    }

    Ok(InteractionModel {
        kind: "manual".to_string(),
        can_auto_grade: false,
        message: "当前题库缺少结构化 Hotspot/Drag Drop 原始数据，已降级为人工自评；未来 SQL 补齐表后可自动上线。".to_string(),
        options: Vec::new(),
        rows: question
            .answer_areas
            .into_iter()
            .map(|row| InteractionRow {
                id: format!("manual-{}", row.sort_order),
                prompt: row.prompt,
                option_group: None,
                correct_selection: Some(row.recommended_selection),
                sort_order: row.sort_order,
            })
            .collect(),
        slots: Vec::new(),
        answer_key: Vec::new(),
    })
}

fn query_interaction_options(conn: &Connection, table: &str, question_id: &str) -> Result<Vec<InteractionOption>, String> {
    if !table_exists(conn, table) {
        return Ok(Vec::new());
    }
    let sql = if table == "interaction_options" || table == "drag_options" {
        format!("SELECT id, option_text, option_group, coalesce(is_distractor, 0), sort_order FROM {table} WHERE question_id = ?1 ORDER BY sort_order")
    } else {
        format!("SELECT id, option_text, option_group, 0, sort_order FROM {table} WHERE question_id = ?1 ORDER BY sort_order")
    };
    let mut stmt = conn.prepare(&sql).map_err(|err| err.to_string())?;
    let options = stmt.query_map(params![question_id], |row| {
        let is_distractor: i64 = row.get(3)?;
        Ok(InteractionOption {
            key: row.get::<_, String>(0)?,
            text: row.get(1)?,
            group: row.get(2)?,
            is_distractor: is_distractor != 0,
            sort_order: row.get(4)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(options)
}

#[derive(Debug)]
struct UnifiedInteractionTarget {
    id: String,
    sort_order: i64,
    label: String,
    option_group: Option<String>,
    correct_option: Option<String>,
}

fn query_unified_interaction_targets(conn: &Connection, question_id: &str) -> Result<Vec<UnifiedInteractionTarget>, String> {
    let mut stmt = conn
        .prepare("SELECT id, position, target_label, option_group, correct_option FROM interaction_targets WHERE question_id = ?1 ORDER BY position")
        .map_err(|err| err.to_string())?;
    let targets = stmt.query_map(params![question_id], |row| {
        Ok(UnifiedInteractionTarget {
            id: row.get(0)?,
            sort_order: row.get(1)?,
            label: row.get(2)?,
            option_group: row.get(3)?,
            correct_option: row.get(4)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(targets)
}

fn question_interaction_kind(question_md: &str) -> Option<String> {
    question_md.lines().find_map(|line| {
        let trimmed = line.trim();
        let value = trimmed.strip_prefix("- Interaction:")?.trim();
        Some(value.to_lowercase().replace(' ', "_").replace('-', "_"))
    })
}

fn query_hotspot_rows(conn: &Connection, question_id: &str) -> Result<Vec<InteractionRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, prompt, option_group, correct_selection, sort_order FROM hotspot_rows WHERE question_id = ?1 ORDER BY sort_order")
        .map_err(|err| err.to_string())?;
    let rows = stmt.query_map(params![question_id], |row| {
        Ok(InteractionRow {
            id: row.get(0)?,
            prompt: row.get(1)?,
            option_group: row.get(2)?,
            correct_selection: row.get(3)?,
            sort_order: row.get(4)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(rows)
}

fn query_drag_slots(conn: &Connection, question_id: &str) -> Result<Vec<InteractionSlot>, String> {
    let mut stmt = conn
        .prepare("SELECT id, slot_label, correct_option, sort_order FROM drag_slots WHERE question_id = ?1 ORDER BY sort_order")
        .map_err(|err| err.to_string())?;
    let slots = stmt.query_map(params![question_id], |row| {
        Ok(InteractionSlot {
            id: row.get(0)?,
            label: row.get(1)?,
            correct_option: row.get(2)?,
            sort_order: row.get(3)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(slots)
}

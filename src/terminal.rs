use crate::protocol::{
    TerminalColor, TerminalDelta, TerminalLine, TerminalRun, TerminalSnapshot,
};

pub fn snapshot_from_parser(
    parser: &vt100::Parser,
    revision: u64,
    exit_status: Option<i32>,
) -> TerminalSnapshot {
    let screen = parser.screen();
    let (rows, cols) = screen.size();
    let (cursor_row, cursor_col) = screen.cursor_position();
    let lines = (0..rows)
        .map(|row| TerminalLine {
            index: row,
            runs: row_runs(screen, row, cols),
            wrapped: screen.row_wrapped(row),
        })
        .collect();
    TerminalSnapshot {
        revision,
        cols,
        rows,
        cursor_row,
        cursor_col,
        cursor_visible: !screen.hide_cursor(),
        title: None,
        lines,
        exit_status,
    }
}

pub fn diff_snapshots(previous: &TerminalSnapshot, next: &TerminalSnapshot) -> Option<TerminalDelta> {
    let lines = next
        .lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| match previous.lines.get(idx) {
            Some(prev) if prev == line => None,
            _ => Some(line.clone()),
        })
        .collect::<Vec<_>>();
    if previous.cols == next.cols
        && previous.rows == next.rows
        && previous.cursor_row == next.cursor_row
        && previous.cursor_col == next.cursor_col
        && previous.cursor_visible == next.cursor_visible
        && previous.title == next.title
        && previous.exit_status == next.exit_status
        && lines.is_empty()
    {
        return None;
    }
    Some(TerminalDelta {
        revision: next.revision,
        cols: next.cols,
        rows: next.rows,
        cursor_row: next.cursor_row,
        cursor_col: next.cursor_col,
        cursor_visible: next.cursor_visible,
        title: next.title.clone(),
        lines,
        exit_status: next.exit_status,
    })
}

fn row_runs(screen: &vt100::Screen, row: u16, cols: u16) -> Vec<TerminalRun> {
    let mut runs = Vec::new();
    let mut current: Option<TerminalRun> = None;
    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            continue;
        };
        if cell.is_wide_continuation() {
            continue;
        }
        let next_run = TerminalRun {
            text: cell_text(cell),
            fg: color_to_wire(cell.fgcolor()),
            bg: color_to_wire(cell.bgcolor()),
            bold: cell.bold(),
            dim: cell.dim(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        };
        match current.as_mut() {
            Some(active) if same_style(active, &next_run) => active.text.push_str(&next_run.text),
            Some(active) => {
                runs.push(active.clone());
                current = Some(next_run);
            }
            None => current = Some(next_run),
        }
    }
    if let Some(active) = current {
        runs.push(active);
    }
    runs
}

fn cell_text(cell: &vt100::Cell) -> String {
    if cell.has_contents() {
        cell.contents().to_string()
    } else {
        " ".to_string()
    }
}

fn same_style(left: &TerminalRun, right: &TerminalRun) -> bool {
    left.fg == right.fg
        && left.bg == right.bg
        && left.bold == right.bold
        && left.dim == right.dim
        && left.italic == right.italic
        && left.underline == right.underline
        && left.inverse == right.inverse
}

fn color_to_wire(color: vt100::Color) -> Option<TerminalColor> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(value) => Some(TerminalColor::Indexed { value }),
        vt100::Color::Rgb(r, g, b) => Some(TerminalColor::Rgb { r, g, b }),
    }
}

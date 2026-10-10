use super::super::{BlockCommentSyntax, CommentSyntax, StringSyntax};
use std::ops::Range;

pub(super) fn source_line_starts(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            content
                .match_indices('\n')
                .map(|(newline_offset, _)| newline_offset + 1),
        )
        .collect()
}

pub(super) fn source_line_at<'a>(
    content: &'a str,
    line_starts: &[usize],
    match_start: usize,
) -> (usize, &'a str, usize) {
    let line_index = line_starts
        .partition_point(|line_start| *line_start <= match_start)
        .saturating_sub(1);
    let line_start = line_starts[line_index];
    let line_end = content[line_start..]
        .find('\n')
        .map(|offset| line_start + offset)
        .unwrap_or(content.len());
    (
        line_index + 1,
        &content[line_start..line_end],
        match_start.saturating_sub(line_start),
    )
}

enum LexicalState<'a> {
    Code,
    String(&'a StringSyntax),
    LineComment {
        start: usize,
    },
    BlockComment {
        syntax: &'a BlockCommentSyntax,
        start: usize,
        depth: usize,
    },
}

fn token_at(bytes: &[u8], offset: usize, token: &str) -> bool {
    bytes[offset..].starts_with(token.as_bytes())
}

fn advance_code<'a>(
    bytes: &[u8],
    offset: &mut usize,
    syntax: &'a CommentSyntax,
) -> LexicalState<'a> {
    if let Some(string) = syntax
        .strings
        .iter()
        .filter(|string| token_at(bytes, *offset, &string.start))
        .max_by_key(|string| string.start.len())
    {
        *offset += string.start.len();
        LexicalState::String(string)
    } else if let Some(block) = syntax
        .block
        .iter()
        .filter(|block| token_at(bytes, *offset, &block.start))
        .max_by_key(|block| block.start.len())
    {
        let start = *offset;
        *offset += block.start.len();
        LexicalState::BlockComment {
            syntax: block,
            start,
            depth: 1,
        }
    } else if let Some(line) = syntax
        .line
        .iter()
        .filter(|line| token_at(bytes, *offset, line))
        .max_by_key(|line| line.len())
    {
        let start = *offset;
        *offset += line.len();
        LexicalState::LineComment { start }
    } else {
        *offset += 1;
        LexicalState::Code
    }
}

fn advance_string<'a>(
    bytes: &[u8],
    offset: &mut usize,
    string: &'a StringSyntax,
) -> LexicalState<'a> {
    if string
        .escape
        .as_deref()
        .is_some_and(|escape| token_at(bytes, *offset, escape))
    {
        *offset += string.escape.as_deref().map_or(0, str::len);
        *offset = (*offset + 1).min(bytes.len());
        LexicalState::String(string)
    } else if token_at(bytes, *offset, &string.end) {
        *offset += string.end.len();
        LexicalState::Code
    } else {
        *offset += 1;
        LexicalState::String(string)
    }
}

fn advance_line_comment<'a>(
    bytes: &[u8],
    offset: &mut usize,
    start: usize,
    ranges: &mut Vec<Range<usize>>,
) -> LexicalState<'a> {
    if bytes[*offset] == b'\n' {
        ranges.push(start..*offset);
        LexicalState::Code
    } else {
        *offset += 1;
        LexicalState::LineComment { start }
    }
}

fn advance_block_comment<'a>(
    bytes: &[u8],
    offset: &mut usize,
    block: &'a BlockCommentSyntax,
    start: usize,
    depth: usize,
    ranges: &mut Vec<Range<usize>>,
) -> LexicalState<'a> {
    if block.nested && token_at(bytes, *offset, &block.start) {
        *offset += block.start.len();
        LexicalState::BlockComment {
            syntax: block,
            start,
            depth: depth + 1,
        }
    } else if token_at(bytes, *offset, &block.end) {
        *offset += block.end.len();
        if depth == 1 {
            ranges.push(start..*offset);
            LexicalState::Code
        } else {
            LexicalState::BlockComment {
                syntax: block,
                start,
                depth: depth - 1,
            }
        }
    } else {
        *offset += 1;
        LexicalState::BlockComment {
            syntax: block,
            start,
            depth,
        }
    }
}

pub(super) fn comment_ranges(content: &str, syntax: &CommentSyntax) -> Vec<Range<usize>> {
    let bytes = content.as_bytes();
    let mut ranges = Vec::new();
    let mut state = LexicalState::Code;
    let mut offset = 0;

    while offset < bytes.len() {
        state = match state {
            LexicalState::Code => advance_code(bytes, &mut offset, syntax),
            LexicalState::String(string) => advance_string(bytes, &mut offset, string),
            LexicalState::LineComment { start } => {
                advance_line_comment(bytes, &mut offset, start, &mut ranges)
            }
            LexicalState::BlockComment {
                syntax: block,
                start,
                depth,
            } => advance_block_comment(bytes, &mut offset, block, start, depth, &mut ranges),
        };
    }

    match state {
        LexicalState::LineComment { start } | LexicalState::BlockComment { start, .. } => {
            ranges.push(start..bytes.len())
        }
        LexicalState::Code | LexicalState::String(_) => {}
    }
    ranges
}

pub(super) fn is_comment_offset(ranges: &[Range<usize>], offset: usize) -> bool {
    let index = ranges.partition_point(|range| range.start <= offset);
    index > 0 && ranges[index - 1].contains(&offset)
}

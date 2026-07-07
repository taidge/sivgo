//! Simplified Unicode BiDi (Bidirectional) text support.
//!
//! Implements a basic version of the Unicode Bidirectional Algorithm (UBA)
//! for mixed LTR/RTL text layout. This handles:
//!
//! - Basic paragraph direction detection (from first strong character)
//! - CSS `direction` and `unicode-bidi` properties
//! - Explicit directional overrides (LRO, RLO, LRE, RLE, PDF)
//! - Glyph reordering for correct visual display
//!
//! This is a simplified implementation that covers common use cases without
//! implementing the full UAX #9 algorithm.

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

/// Text direction (CSS `direction` property).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left-to-right (default for Latin, Cyrillic, etc.).
    #[default]
    Ltr,
    /// Right-to-left (for Arabic, Hebrew, etc.).
    Rtl,
}

impl Direction {
    /// Parse a CSS `direction` property string.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "rtl" => Direction::Rtl,
            _ => Direction::Ltr,
        }
    }

    /// The embedding level for this direction (0 for LTR, 1 for RTL).
    pub fn level(self) -> u8 {
        match self {
            Direction::Ltr => 0,
            Direction::Rtl => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// UnicodeBidi
// ---------------------------------------------------------------------------

/// CSS `unicode-bidi` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnicodeBidi {
    /// Normal BiDi algorithm behavior.
    #[default]
    Normal,
    /// Open a new embedding level (LRE or RLE depending on direction).
    Embed,
    /// Override all characters to the specified direction (LRO or RLO).
    BidiOverride,
    /// Isolate the element from surrounding text for BiDi purposes.
    Isolate,
    /// Combination of override and isolate.
    IsolateOverride,
    /// Use plaintext heuristics for paragraph direction.
    Plaintext,
}

impl UnicodeBidi {
    /// Parse a CSS `unicode-bidi` property string.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "embed" => UnicodeBidi::Embed,
            "bidi-override" => UnicodeBidi::BidiOverride,
            "isolate" => UnicodeBidi::Isolate,
            "isolate-override" => UnicodeBidi::IsolateOverride,
            "plaintext" => UnicodeBidi::Plaintext,
            _ => UnicodeBidi::Normal,
        }
    }
}

// ---------------------------------------------------------------------------
// BidiCharType - simplified character classification
// ---------------------------------------------------------------------------

/// Simplified BiDi character type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiCharType {
    /// Strong left-to-right (Latin, Greek, Cyrillic, CJK, etc.).
    StrongLtr,
    /// Strong right-to-left (Arabic, Hebrew, etc.).
    StrongRtl,
    /// Weak character (digits, punctuation, spaces, etc.).
    Weak,
    /// Explicit LTR override / embedding.
    ExplicitLtr,
    /// Explicit RTL override / embedding.
    ExplicitRtl,
    /// Pop directional formatting.
    PopDirectional,
}

/// Classify a character's BiDi type.
///
/// This is a simplified classification covering the most common scripts.
pub fn classify_bidi_char(ch: char) -> BidiCharType {
    let cp = ch as u32;
    match cp {
        // Explicit formatting characters
        0x202A => BidiCharType::ExplicitLtr,    // LRE
        0x202B => BidiCharType::ExplicitRtl,    // RLE
        0x202D => BidiCharType::ExplicitLtr,    // LRO
        0x202E => BidiCharType::ExplicitRtl,    // RLO
        0x202C => BidiCharType::PopDirectional, // PDF
        0x2066 => BidiCharType::ExplicitLtr,    // LRI
        0x2067 => BidiCharType::ExplicitRtl,    // RLI
        0x2068 => BidiCharType::Weak,           // FSI (treat as weak for simplification)
        0x2069 => BidiCharType::PopDirectional, // PDI

        // Arabic block
        0x0600..=0x06FF => BidiCharType::StrongRtl,
        // Arabic Supplement
        0x0750..=0x077F => BidiCharType::StrongRtl,
        // Arabic Extended-A
        0x08A0..=0x08FF => BidiCharType::StrongRtl,
        // Arabic Presentation Forms-A
        0xFB50..=0xFDFF => BidiCharType::StrongRtl,
        // Arabic Presentation Forms-B
        0xFE70..=0xFEFF => BidiCharType::StrongRtl,
        // Hebrew
        0x0590..=0x05FF => BidiCharType::StrongRtl,
        // Hebrew Presentation Forms
        0xFB1D..=0xFB4F => BidiCharType::StrongRtl,
        // Syriac
        0x0700..=0x074F => BidiCharType::StrongRtl,
        // Thaana
        0x0780..=0x07BF => BidiCharType::StrongRtl,
        // NKo
        0x07C0..=0x07FF => BidiCharType::StrongRtl,
        // Samaritan
        0x0800..=0x083F => BidiCharType::StrongRtl,
        // Mandaic
        0x0840..=0x085F => BidiCharType::StrongRtl,

        // European digits and common punctuation -> weak
        0x0030..=0x0039 => BidiCharType::Weak, // 0-9
        0x0020 => BidiCharType::Weak,          // space
        0x0009 => BidiCharType::Weak,          // tab
        0x000A | 0x000D => BidiCharType::Weak, // newlines

        // Common punctuation
        0x0021..=0x002F => BidiCharType::Weak,
        0x003A..=0x0040 => BidiCharType::Weak,
        0x005B..=0x0060 => BidiCharType::Weak,
        0x007B..=0x007E => BidiCharType::Weak,

        // Latin, Greek, Cyrillic and many other LTR scripts
        0x0041..=0x005A => BidiCharType::StrongLtr, // A-Z
        0x0061..=0x007A => BidiCharType::StrongLtr, // a-z
        0x00C0..=0x024F => BidiCharType::StrongLtr, // Latin Extended
        0x0370..=0x03FF => BidiCharType::StrongLtr, // Greek
        0x0400..=0x04FF => BidiCharType::StrongLtr, // Cyrillic
        0x0500..=0x052F => BidiCharType::StrongLtr, // Cyrillic Supplement
        0x1E00..=0x1EFF => BidiCharType::StrongLtr, // Latin Extended Additional
        0x2C60..=0x2C7F => BidiCharType::StrongLtr, // Latin Extended-C

        // CJK
        0x3000..=0x9FFF => BidiCharType::StrongLtr,
        0xF900..=0xFAFF => BidiCharType::StrongLtr,

        // Default: treat as weak (neutral)
        _ => BidiCharType::Weak,
    }
}

// ---------------------------------------------------------------------------
// BidiRun
// ---------------------------------------------------------------------------

/// A run of characters at the same embedding level / direction.
#[derive(Debug, Clone)]
pub struct BidiRun {
    /// Start index (character offset) in the original string.
    pub start: usize,
    /// End index (exclusive) in the original string.
    pub end: usize,
    /// The resolved embedding level for this run.
    pub level: u8,
    /// Whether this run should be displayed right-to-left.
    pub is_rtl: bool,
}

// ---------------------------------------------------------------------------
// BidiInfo
// ---------------------------------------------------------------------------

/// Result of BiDi analysis for a text string.
#[derive(Debug, Clone)]
pub struct BidiInfo {
    /// The original text.
    pub text: String,
    /// The resolved paragraph direction.
    pub paragraph_direction: Direction,
    /// The per-character embedding levels.
    pub levels: Vec<u8>,
    /// Runs of characters at the same level.
    pub runs: Vec<BidiRun>,
    /// Visual reordering: maps visual index -> logical index.
    pub visual_order: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyze a text string for BiDi layout.
///
/// This performs a simplified Unicode BiDi analysis:
///
/// 1. Determine paragraph direction (from `direction` property or first strong char).
/// 2. Assign embedding levels based on character types and overrides.
/// 3. Compute visual reordering.
///
/// # Arguments
/// * `text` - The text to analyze.
/// * `direction` - CSS `direction` property value.
/// * `unicode_bidi` - CSS `unicode-bidi` property value.
pub fn analyze_bidi(text: &str, direction: Direction, unicode_bidi: UnicodeBidi) -> BidiInfo {
    if text.is_empty() {
        return BidiInfo {
            text: String::new(),
            paragraph_direction: direction,
            levels: Vec::new(),
            runs: Vec::new(),
            visual_order: Vec::new(),
        };
    }

    let chars: Vec<char> = text.chars().collect();

    // Determine paragraph embedding level.
    let para_level = match unicode_bidi {
        UnicodeBidi::Plaintext => {
            // Detect from first strong character.
            detect_paragraph_direction(&chars).level()
        }
        _ => direction.level(),
    };

    // Assign embedding levels.
    let levels = assign_levels(&chars, para_level, unicode_bidi, direction);

    // Compute runs.
    let runs = compute_runs(&levels);

    // Compute visual reordering.
    let visual_order = compute_visual_order(&levels);

    BidiInfo {
        text: text.to_string(),
        paragraph_direction: direction,
        levels,
        runs,
        visual_order,
    }
}

/// Reorder a slice of items (typically glyph positions) according to BiDi visual order.
///
/// Given a slice of items and a `BidiInfo`, returns a new `Vec` with items
/// reordered for visual display. Within RTL runs, items are reversed.
pub fn reorder_by_bidi<T: Clone>(items: &[T], bidi: &BidiInfo) -> Vec<T> {
    if items.is_empty() || bidi.visual_order.is_empty() {
        return items.to_vec();
    }

    let n = items.len().min(bidi.visual_order.len());
    let mut result = Vec::with_capacity(n);

    for &logical_idx in &bidi.visual_order[..n] {
        if logical_idx < items.len() {
            result.push(items[logical_idx].clone());
        }
    }

    result
}

/// Detect the paragraph direction from the first strong character.
pub fn detect_paragraph_direction(chars: &[char]) -> Direction {
    for &ch in chars {
        match classify_bidi_char(ch) {
            BidiCharType::StrongLtr => return Direction::Ltr,
            BidiCharType::StrongRtl => return Direction::Rtl,
            _ => {}
        }
    }
    // Default to LTR if no strong character found.
    Direction::Ltr
}

// ---------------------------------------------------------------------------
// Internal: level assignment
// ---------------------------------------------------------------------------

/// Assign embedding levels to each character.
///
/// This is a simplified version of the UAX #9 algorithm. It handles:
/// - Base paragraph level
/// - Explicit overrides (bidi-override)
/// - Strong character types
/// - Weak/neutral resolution based on surrounding context
fn assign_levels(
    chars: &[char],
    para_level: u8,
    unicode_bidi: UnicodeBidi,
    direction: Direction,
) -> Vec<u8> {
    let mut levels = vec![para_level; chars.len()];

    match unicode_bidi {
        UnicodeBidi::BidiOverride | UnicodeBidi::IsolateOverride => {
            // Override: all characters get the specified direction.
            let override_level = direction.level();
            // Make sure the override level has the right parity.
            // Even levels are LTR, odd levels are RTL.
            for level in &mut levels {
                *level = if override_level == 1 {
                    // RTL override: use next odd level at or above para_level.
                    para_level | 1
                } else {
                    // LTR override: use next even level at or above para_level.
                    (para_level + 1) & !1
                };
                // Ensure minimum is the appropriate level.
                if *level < override_level {
                    *level = override_level;
                }
            }
        }
        UnicodeBidi::Embed => {
            // Embedding: increase level for the content.
            let embed_level = if direction == Direction::Rtl {
                para_level | 1 // next odd
            } else {
                (para_level + 1) & !1 // next even (or keep if already even)
            };
            // Ensure we actually increase.
            let embed_level = if embed_level <= para_level {
                para_level + if direction == Direction::Rtl { 1 } else { 2 }
            } else {
                embed_level
            };
            levels.fill(embed_level);
        }
        _ => {
            // Normal / Isolate / Plaintext: resolve based on character types.
            resolve_levels_from_chars(chars, &mut levels, para_level);
        }
    }

    levels
}

/// Resolve embedding levels from character BiDi types.
///
/// Simplified resolution:
/// - Strong LTR characters get an even level (>= para_level).
/// - Strong RTL characters get an odd level (>= para_level).
/// - Weak/neutral characters inherit from surrounding strong characters,
///   or default to the paragraph level.
fn resolve_levels_from_chars(chars: &[char], levels: &mut [u8], para_level: u8) {
    let n = chars.len();

    // Pass 1: Assign levels to strong characters.
    let mut explicit_stack: Vec<(u8, bool)> = Vec::new(); // (level, is_override)
    let mut current_level = para_level;

    for (i, &ch) in chars.iter().enumerate() {
        let bidi_type = classify_bidi_char(ch);

        match bidi_type {
            BidiCharType::ExplicitLtr => {
                explicit_stack.push((current_level, false));
                // Next higher even level.
                current_level = (current_level + 2) & !1;
                if current_level > 125 {
                    current_level = explicit_stack.last().map_or(para_level, |&(l, _)| l);
                    explicit_stack.pop();
                }
                levels[i] = current_level;
            }
            BidiCharType::ExplicitRtl => {
                explicit_stack.push((current_level, false));
                // Next higher odd level.
                current_level = (current_level + 1) | 1;
                if current_level > 125 {
                    current_level = explicit_stack.last().map_or(para_level, |&(l, _)| l);
                    explicit_stack.pop();
                }
                levels[i] = current_level;
            }
            BidiCharType::PopDirectional => {
                if let Some((prev_level, _)) = explicit_stack.pop() {
                    current_level = prev_level;
                }
                levels[i] = current_level;
            }
            BidiCharType::StrongLtr => {
                // Even level at or above current.
                let ltr_level = current_level & !1; // round down to even
                levels[i] = if ltr_level >= current_level {
                    ltr_level
                } else {
                    current_level
                };
                // Ensure even.
                if !levels[i].is_multiple_of(2) {
                    levels[i] = (levels[i] + 1) & !1;
                }
            }
            BidiCharType::StrongRtl => {
                // Odd level at or above current.
                levels[i] = current_level | 1;
            }
            BidiCharType::Weak => {
                levels[i] = current_level;
            }
        }
    }

    // Pass 2: Resolve weak/neutral characters based on surrounding strong chars.
    // Find runs of weak characters and set them to the higher of the
    // surrounding strong levels.
    let mut i = 0;
    while i < n {
        if classify_bidi_char(chars[i]) == BidiCharType::Weak {
            let run_start = i;
            while i < n && classify_bidi_char(chars[i]) == BidiCharType::Weak {
                i += 1;
            }
            let run_end = i;

            // Find preceding strong level.
            let prev_level = if run_start > 0 {
                levels[run_start - 1]
            } else {
                para_level
            };

            // Find following strong level.
            let next_level = if run_end < n {
                levels[run_end]
            } else {
                para_level
            };

            // Use the higher of the two surrounding levels (simplified rule N1/N2).
            let resolved_level = prev_level.max(next_level);
            for level in levels.iter_mut().take(run_end).skip(run_start) {
                *level = resolved_level;
            }
        } else {
            i += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: run computation
// ---------------------------------------------------------------------------

/// Compute runs of consecutive characters at the same embedding level.
fn compute_runs(levels: &[u8]) -> Vec<BidiRun> {
    if levels.is_empty() {
        return Vec::new();
    }

    let mut runs = Vec::new();
    let mut run_start = 0;
    let mut run_level = levels[0];

    for (i, &level) in levels.iter().enumerate().skip(1) {
        if level != run_level {
            runs.push(BidiRun {
                start: run_start,
                end: i,
                level: run_level,
                is_rtl: !run_level.is_multiple_of(2),
            });
            run_start = i;
            run_level = level;
        }
    }

    // Final run.
    runs.push(BidiRun {
        start: run_start,
        end: levels.len(),
        level: run_level,
        is_rtl: !run_level.is_multiple_of(2),
    });

    runs
}

// ---------------------------------------------------------------------------
// Internal: visual reordering
// ---------------------------------------------------------------------------

/// Compute visual order from embedding levels.
///
/// This implements a simplified version of the L2 rule from UAX #9:
/// reverse the order of characters at each level, from the highest level
/// down to the lowest odd level.
fn compute_visual_order(levels: &[u8]) -> Vec<usize> {
    let n = levels.len();
    if n == 0 {
        return Vec::new();
    }

    // Start with logical order.
    let mut order: Vec<usize> = (0..n).collect();

    // Find the max level and the lowest odd level.
    let max_level = *levels.iter().max().unwrap_or(&0);
    let min_odd_level = levels
        .iter()
        .copied()
        .filter(|&l| !l.is_multiple_of(2))
        .min()
        .unwrap_or(max_level + 1);

    if min_odd_level > max_level {
        // No RTL content at all.
        return order;
    }

    // For each level from max_level down to min_odd_level,
    // reverse any contiguous sequence of characters at that level or higher.
    for level in (min_odd_level..=max_level).rev() {
        let mut i = 0;
        while i < n {
            if levels[order[i]] >= level {
                // Find the end of this run at or above this level.
                let run_start = i;
                while i < n && levels[order[i]] >= level {
                    i += 1;
                }
                // Reverse the run in the visual order.
                order[run_start..i].reverse();
            } else {
                i += 1;
            }
        }
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Direction
    // -----------------------------------------------------------------------

    #[test]
    fn test_direction_parse() {
        assert_eq!(Direction::parse("ltr"), Direction::Ltr);
        assert_eq!(Direction::parse("rtl"), Direction::Rtl);
        assert_eq!(Direction::parse("LTR"), Direction::Ltr);
        assert_eq!(Direction::parse("RTL"), Direction::Rtl);
        assert_eq!(Direction::parse("unknown"), Direction::Ltr);
    }

    #[test]
    fn test_direction_level() {
        assert_eq!(Direction::Ltr.level(), 0);
        assert_eq!(Direction::Rtl.level(), 1);
    }

    // -----------------------------------------------------------------------
    // UnicodeBidi
    // -----------------------------------------------------------------------

    #[test]
    fn test_unicode_bidi_parse() {
        assert_eq!(UnicodeBidi::parse("normal"), UnicodeBidi::Normal);
        assert_eq!(UnicodeBidi::parse("embed"), UnicodeBidi::Embed);
        assert_eq!(
            UnicodeBidi::parse("bidi-override"),
            UnicodeBidi::BidiOverride
        );
        assert_eq!(UnicodeBidi::parse("isolate"), UnicodeBidi::Isolate);
        assert_eq!(
            UnicodeBidi::parse("isolate-override"),
            UnicodeBidi::IsolateOverride
        );
        assert_eq!(UnicodeBidi::parse("plaintext"), UnicodeBidi::Plaintext);
        assert_eq!(UnicodeBidi::parse("unknown"), UnicodeBidi::Normal);
    }

    // -----------------------------------------------------------------------
    // Character classification
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_latin_is_strong_ltr() {
        assert_eq!(classify_bidi_char('A'), BidiCharType::StrongLtr);
        assert_eq!(classify_bidi_char('z'), BidiCharType::StrongLtr);
    }

    #[test]
    fn test_classify_arabic_is_strong_rtl() {
        // Arabic letter Alef
        assert_eq!(classify_bidi_char('\u{0627}'), BidiCharType::StrongRtl);
        // Arabic letter Ba
        assert_eq!(classify_bidi_char('\u{0628}'), BidiCharType::StrongRtl);
    }

    #[test]
    fn test_classify_hebrew_is_strong_rtl() {
        // Hebrew letter Alef
        assert_eq!(classify_bidi_char('\u{05D0}'), BidiCharType::StrongRtl);
    }

    #[test]
    fn test_classify_digits_are_weak() {
        assert_eq!(classify_bidi_char('0'), BidiCharType::Weak);
        assert_eq!(classify_bidi_char('9'), BidiCharType::Weak);
    }

    #[test]
    fn test_classify_space_is_weak() {
        assert_eq!(classify_bidi_char(' '), BidiCharType::Weak);
    }

    #[test]
    fn test_classify_explicit_markers() {
        assert_eq!(classify_bidi_char('\u{202A}'), BidiCharType::ExplicitLtr); // LRE
        assert_eq!(classify_bidi_char('\u{202B}'), BidiCharType::ExplicitRtl); // RLE
        assert_eq!(classify_bidi_char('\u{202C}'), BidiCharType::PopDirectional); // PDF
        assert_eq!(classify_bidi_char('\u{202D}'), BidiCharType::ExplicitLtr); // LRO
        assert_eq!(classify_bidi_char('\u{202E}'), BidiCharType::ExplicitRtl); // RLO
    }

    // -----------------------------------------------------------------------
    // Paragraph direction detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_paragraph_direction_ltr() {
        let chars: Vec<char> = "Hello".chars().collect();
        assert_eq!(detect_paragraph_direction(&chars), Direction::Ltr);
    }

    #[test]
    fn test_detect_paragraph_direction_rtl() {
        // Arabic text
        let chars: Vec<char> = "\u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064A}\u{0629}"
            .chars()
            .collect();
        assert_eq!(detect_paragraph_direction(&chars), Direction::Rtl);
    }

    #[test]
    fn test_detect_paragraph_direction_starts_with_number() {
        // "123Hello" - numbers are weak, first strong char is 'H' (LTR).
        let chars: Vec<char> = "123Hello".chars().collect();
        assert_eq!(detect_paragraph_direction(&chars), Direction::Ltr);
    }

    #[test]
    fn test_detect_paragraph_direction_empty() {
        assert_eq!(detect_paragraph_direction(&[]), Direction::Ltr);
    }

    // -----------------------------------------------------------------------
    // BiDi analysis
    // -----------------------------------------------------------------------

    #[test]
    fn test_analyze_bidi_empty() {
        let info = analyze_bidi("", Direction::Ltr, UnicodeBidi::Normal);
        assert!(info.levels.is_empty());
        assert!(info.runs.is_empty());
        assert!(info.visual_order.is_empty());
    }

    #[test]
    fn test_analyze_bidi_pure_ltr() {
        let info = analyze_bidi("Hello", Direction::Ltr, UnicodeBidi::Normal);
        assert_eq!(info.levels.len(), 5);
        // All characters should have even (LTR) levels.
        for &level in &info.levels {
            assert_eq!(level % 2, 0, "LTR text should have even levels");
        }
        // Visual order should be logical order.
        assert_eq!(info.visual_order, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_analyze_bidi_pure_rtl() {
        // Arabic text "العربية"
        let text = "\u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064A}\u{0629}";
        let info = analyze_bidi(text, Direction::Rtl, UnicodeBidi::Normal);
        assert_eq!(info.levels.len(), text.chars().count());
        // All characters should have odd (RTL) levels.
        for &level in &info.levels {
            assert_eq!(level % 2, 1, "RTL text should have odd levels");
        }
        // Visual order should be reversed.
        let n = info.visual_order.len();
        for (vis_i, &log_i) in info.visual_order.iter().enumerate() {
            assert_eq!(log_i, n - 1 - vis_i, "RTL visual order should be reversed");
        }
    }

    #[test]
    fn test_analyze_bidi_override_rtl() {
        let info = analyze_bidi("Hello", Direction::Rtl, UnicodeBidi::BidiOverride);
        // All characters should be forced to RTL (odd level).
        for &level in &info.levels {
            assert_eq!(level % 2, 1, "Override RTL should force odd levels");
        }
        // Visual order should be reversed.
        let n = info.visual_order.len();
        for (vis_i, &log_i) in info.visual_order.iter().enumerate() {
            assert_eq!(log_i, n - 1 - vis_i, "Override RTL should reverse order");
        }
    }

    #[test]
    fn test_analyze_bidi_override_ltr() {
        // Force Arabic text to LTR display.
        let text = "\u{0627}\u{0644}\u{0639}";
        let info = analyze_bidi(text, Direction::Ltr, UnicodeBidi::BidiOverride);
        // All characters should have even (LTR) levels.
        for &level in &info.levels {
            assert_eq!(level % 2, 0, "Override LTR should force even levels");
        }
        // Visual order should be logical (not reversed).
        assert_eq!(info.visual_order, vec![0, 1, 2]);
    }

    #[test]
    fn test_analyze_bidi_runs_computed() {
        let info = analyze_bidi("Hello", Direction::Ltr, UnicodeBidi::Normal);
        assert!(!info.runs.is_empty(), "Should have at least one run");
        // Pure LTR text should be a single run.
        assert_eq!(info.runs.len(), 1);
        assert_eq!(info.runs[0].start, 0);
        assert_eq!(info.runs[0].end, 5);
        assert!(!info.runs[0].is_rtl);
    }

    #[test]
    fn test_analyze_bidi_plaintext_detects_direction() {
        // Arabic text should auto-detect RTL.
        let text = "\u{0627}\u{0644}\u{0639}";
        let info = analyze_bidi(text, Direction::Ltr, UnicodeBidi::Plaintext);
        // Even though direction says LTR, plaintext mode should detect RTL.
        for &level in &info.levels {
            assert_eq!(level % 2, 1, "Plaintext with Arabic text should detect RTL");
        }
    }

    // -----------------------------------------------------------------------
    // Reordering
    // -----------------------------------------------------------------------

    #[test]
    fn test_reorder_ltr() {
        let items = vec!['H', 'e', 'l', 'l', 'o'];
        let bidi = analyze_bidi("Hello", Direction::Ltr, UnicodeBidi::Normal);
        let reordered = reorder_by_bidi(&items, &bidi);
        assert_eq!(reordered, vec!['H', 'e', 'l', 'l', 'o']);
    }

    #[test]
    fn test_reorder_rtl_override() {
        let items = vec!['H', 'e', 'l', 'l', 'o'];
        let bidi = analyze_bidi("Hello", Direction::Rtl, UnicodeBidi::BidiOverride);
        let reordered = reorder_by_bidi(&items, &bidi);
        assert_eq!(reordered, vec!['o', 'l', 'l', 'e', 'H']);
    }

    #[test]
    fn test_reorder_empty() {
        let items: Vec<char> = Vec::new();
        let bidi = analyze_bidi("", Direction::Ltr, UnicodeBidi::Normal);
        let reordered = reorder_by_bidi(&items, &bidi);
        assert!(reordered.is_empty());
    }

    #[test]
    fn test_reorder_mixed_ltr_rtl() {
        // "Hello" followed by 3 Arabic chars in an LTR paragraph.
        // The Arabic portion should be reversed in visual order.
        let text = "Hi\u{0627}\u{0628}\u{0629}";
        let bidi = analyze_bidi(text, Direction::Ltr, UnicodeBidi::Normal);
        let chars: Vec<char> = text.chars().collect();
        let reordered = reorder_by_bidi(&chars, &bidi);

        // The LTR part (H, i) should come first in visual order.
        assert_eq!(reordered[0], 'H');
        assert_eq!(reordered[1], 'i');
        // The RTL part should be reversed.
        assert_eq!(reordered[2], '\u{0629}');
        assert_eq!(reordered[3], '\u{0628}');
        assert_eq!(reordered[4], '\u{0627}');
    }

    // -----------------------------------------------------------------------
    // Visual order computation
    // -----------------------------------------------------------------------

    #[test]
    fn test_visual_order_all_ltr() {
        let levels = vec![0, 0, 0, 0];
        let order = compute_visual_order(&levels);
        assert_eq!(order, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_visual_order_all_rtl() {
        let levels = vec![1, 1, 1, 1];
        let order = compute_visual_order(&levels);
        assert_eq!(order, vec![3, 2, 1, 0]);
    }

    #[test]
    fn test_visual_order_mixed() {
        // LTR LTR RTL RTL LTR
        let levels = vec![0, 0, 1, 1, 0];
        let order = compute_visual_order(&levels);
        // LTR portion stays, RTL portion is reversed.
        assert_eq!(order, vec![0, 1, 3, 2, 4]);
    }

    #[test]
    fn test_visual_order_empty() {
        let order = compute_visual_order(&[]);
        assert!(order.is_empty());
    }

    // -----------------------------------------------------------------------
    // Run computation
    // -----------------------------------------------------------------------

    #[test]
    fn test_compute_runs_single() {
        let levels = vec![0, 0, 0];
        let runs = compute_runs(&levels);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].start, 0);
        assert_eq!(runs[0].end, 3);
        assert!(!runs[0].is_rtl);
    }

    #[test]
    fn test_compute_runs_mixed() {
        let levels = vec![0, 0, 1, 1, 0];
        let runs = compute_runs(&levels);
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].start, 0);
        assert_eq!(runs[0].end, 2);
        assert!(!runs[0].is_rtl);
        assert_eq!(runs[1].start, 2);
        assert_eq!(runs[1].end, 4);
        assert!(runs[1].is_rtl);
        assert_eq!(runs[2].start, 4);
        assert_eq!(runs[2].end, 5);
        assert!(!runs[2].is_rtl);
    }

    #[test]
    fn test_compute_runs_empty() {
        let runs = compute_runs(&[]);
        assert!(runs.is_empty());
    }
}

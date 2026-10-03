//! 대사 블록 병합 번역.
//!
//! 원문은 문장 중간에서 줄이 끊기는 경우가 많아 줄마다 따로 번역하면 어색해진다.
//! 블록의 줄들을 이어 한 번에 번역한 뒤, 번역문을 띄어쓰기 기준으로 원래 줄 수 안에 나눠 담는다.
//! 401 커맨드를 늘릴 수는 없으므로 줄 수는 원문을 넘지 않고, 남는 줄은 빈 문자열이 된다.

use super::codes::display_width;

/// 단어 사이 공백의 폭
const SPACE: f32 = 0.5;
/// 부동소수점 오차 허용치
const EPSILON: f32 = 1e-3;

/// 띄어쓰기로 어절을 나누는 언어인지. 병합 번역은 이런 언어에서만 쓸 수 있다.
pub fn splits_by_space(language: &str) -> bool {
    let language = language.to_ascii_lowercase();
    !["japanese", "chinese", "thai"].iter().any(|l| language.contains(l))
}

/// 원문 줄들을 하나로 잇는다. 경계 양쪽이 모두 띄어쓰기를 쓰는 문자일 때만 공백을 넣는다 (일본어는 그냥 붙인다).
pub fn join(lines: &[&str]) -> String {
    let mut out = String::new();
    for line in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        if let (Some(a), Some(b)) = (out.chars().last(), line.chars().next()) {
            if !is_cjk(a) && !is_cjk(b) {
                out.push(' ');
            }
        }
        out.push_str(line);
    }
    out
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{30FF}' // CJK 기호, 히라가나, 가타카나
        | '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{FF00}'..='\u{FFEF}' // 전각/반각 형태
    )
}

/// 번역문을 어절 단위로 최대 `n`줄에 나눈다.
/// 한 줄 폭이 `limit`을 넘지 않는 가장 적은 줄 수를 고르고, 그 줄 수 안에서는 줄 길이를 고르게 맞춘다.
/// `n`줄로도 넘치면 `n`줄에 최대한 고르게 나눈다. 결과는 항상 `n`개이고 남는 줄은 빈 문자열이다.
pub fn split(text: &str, n: usize, limit: f32) -> Vec<String> {
    let mut lines = vec![String::new(); n];
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() || n == 0 {
        return lines;
    }
    let widths: Vec<f32> = words.iter().map(|w| display_width(w)).collect();
    let max_lines = n.min(words.len());
    let ends = (1..=max_lines)
        .find_map(|k| partition(&widths, k, Some(limit)))
        .or_else(|| partition(&widths, max_lines, None))
        .expect("폭 제한이 없으면 항상 나눌 수 있다");
    let mut start = 0;
    for (line, end) in lines.iter_mut().zip(ends) {
        *line = words[start..end].join(" ");
        start = end;
    }
    lines
}

/// 단어들을 정확히 `k`줄로 나눴을 때 각 줄의 끝 인덱스. 줄 폭의 제곱합을 최소화해 길이를 고르게 한다.
/// `limit`이 있으면 그보다 넓은 줄은 허용하지 않는다 (단어 하나만으로 이미 넓은 줄은 예외).
fn partition(widths: &[f32], k: usize, limit: Option<f32>) -> Option<Vec<usize>> {
    let n = widths.len();
    let line_width = |a: usize, b: usize| widths[a..b].iter().sum::<f32>() + SPACE * (b - a - 1) as f32;
    // cost[j][m]: 앞 j단어를 m줄에 담는 최소 비용, prev[j][m]: 그때 마지막 줄의 시작 인덱스
    let mut cost = vec![vec![f32::INFINITY; k + 1]; n + 1];
    let mut prev = vec![vec![0usize; k + 1]; n + 1];
    cost[0][0] = 0.0;
    for m in 1..=k {
        for j in m..=n {
            for i in m - 1..j {
                if cost[i][m - 1].is_infinite() {
                    continue;
                }
                let w = line_width(i, j);
                if limit.is_some_and(|l| w > l + EPSILON && j - i > 1) {
                    continue;
                }
                let c = cost[i][m - 1] + w * w;
                if c < cost[j][m] {
                    cost[j][m] = c;
                    prev[j][m] = i;
                }
            }
        }
    }
    if cost[n][k].is_infinite() {
        return None;
    }
    let mut ends = vec![0; k];
    let mut j = n;
    for m in (1..=k).rev() {
        ends[m - 1] = j;
        j = prev[j][m];
    }
    Some(ends)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_by_script() {
        assert_eq!(join(&["ハロルドは昨日、", "村の外れで", ""]), "ハロルドは昨日、村の外れで");
        assert_eq!(join(&["I saw a strange ", "light yesterday."]), "I saw a strange light yesterday.");
        assert_eq!(join(&["\\C[2]光\\C[0]", "を見た"]), "\\C[2]光\\C[0]を見た");
    }

    #[test]
    fn uses_fewest_lines_and_balances() {
        // 한 줄에 들어가면 한 줄만 쓰고 나머지는 비운다
        assert_eq!(split("짧은 대사", 3, 20.0), ["짧은 대사", "", ""]);
        // 두 줄이 필요하면 고르게 나눈다 (앞줄만 꽉 채우지 않음)
        let lines = split("하나 둘 셋 넷 다섯 여섯", 4, 10.0);
        assert_eq!(lines, ["하나 둘 셋", "넷 다섯 여섯", "", ""]);
    }

    #[test]
    fn overflows_into_last_resort() {
        // n줄로도 넘치면 n줄에 고르게 담는다
        let lines = split("가나다라 마바사아 자차카타 파하가나", 2, 4.0);
        assert_eq!(lines, ["가나다라 마바사아", "자차카타 파하가나"]);
        // 단어 수가 줄 수보다 적으면 남는 줄은 비운다
        assert_eq!(split("아주긴단어하나뿐", 2, 3.0), ["아주긴단어하나뿐", ""]);
        assert_eq!(split("  ", 2, 3.0), ["", ""]);
    }

    #[test]
    fn keeps_codes_attached() {
        let lines = split("\\C[2]해롤드\\C[0]는 어제 마을 밖에서 \\I[5]이상한 빛을 봤다고 했다.\\!", 3, 13.0);
        assert_eq!(lines, ["\\C[2]해롤드\\C[0]는 어제 마을 밖에서", "\\I[5]이상한 빛을 봤다고 했다.\\!", ""]);
    }

    #[test]
    fn language_check() {
        assert!(splits_by_space("Korean"));
        assert!(!splits_by_space("Japanese"));
        assert!(!splits_by_space("Simplified Chinese"));
    }
}

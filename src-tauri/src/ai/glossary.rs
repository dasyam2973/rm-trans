//! 단어장: 배치에 실제로 등장하는 용어만 골라 AI 요청에 넣는다.

use crate::store::GlossaryTerm;

/// 매칭된 용어를 지워 둘 자리. 원문에 나올 일이 없는 문자를 쓴다.
const ERASED: char = '\u{0}';

/// 매칭 준비를 마친 단어장. 긴 용어부터 매칭해야 "ハロ"가 "ハロルド" 안에서 따로 잡히지 않는다.
pub struct Glossary {
    terms: Vec<GlossaryTerm>,
}

impl Glossary {
    /// 원문이나 번역이 비어 있는 항목은 버리고, 같은 원문은 앞의 것만 남긴다.
    pub fn new(terms: Vec<GlossaryTerm>) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut terms: Vec<GlossaryTerm> = terms
            .into_iter()
            .filter(|t| !t.source.trim().is_empty() && !t.target.trim().is_empty())
            .filter(|t| seen.insert(t.source.clone()))
            .collect();
        // 정렬이 안정적이라 같은 길이끼리는 사용자가 넣은 순서가 유지된다
        terms.sort_by_key(|t| std::cmp::Reverse(t.source.chars().count()));
        Self { terms }
    }

    /// 텍스트들에 등장하는 용어를 단어장 순서(긴 것부터)대로 돌려준다.
    /// 일본어/중국어처럼 띄어쓰기가 없는 언어를 위해 단어 경계 없이 부분 문자열로 찾는다.
    pub fn matching<'a, I>(&self, texts: I) -> Vec<&GlossaryTerm>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut rest: Vec<String> = texts.into_iter().map(str::to_owned).collect();
        let mut found = Vec::new();
        for term in &self.terms {
            let mut hit = false;
            for text in rest.iter_mut() {
                if text.contains(&term.source) {
                    hit = true;
                    // 더 짧은 용어가 이 부분에서 다시 잡히지 않도록 지운다
                    *text = text.replace(&term.source, &ERASED.to_string());
                }
            }
            if hit {
                found.push(term);
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(source: &str, target: &str) -> GlossaryTerm {
        GlossaryTerm { source: source.into(), target: target.into(), note: None }
    }

    #[test]
    fn longest_match_wins() {
        let g = Glossary::new(vec![term("ハロ", "하로"), term("ハロルド", "해롤드"), term("村", "마을")]);
        let found: Vec<_> = g.matching(["ハロルドさん！"]).iter().map(|t| t.target.as_str()).collect();
        assert_eq!(found, ["해롤드"]);
        let found: Vec<_> = g.matching(["ハロルドとハロが村に"]).iter().map(|t| t.target.as_str()).collect();
        assert_eq!(found, ["해롤드", "하로", "마을"]);
    }

    #[test]
    fn skips_empty_and_duplicates() {
        let g = Glossary::new(vec![term("勇者", "용사"), term("勇者", "영웅"), term("魔王", " "), term("", "x")]);
        let found: Vec<_> = g.matching(["勇者と魔王"]).iter().map(|t| t.target.as_str()).collect();
        assert_eq!(found, ["용사"]);
    }
}

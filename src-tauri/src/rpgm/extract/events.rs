//! 이벤트 커맨드 리스트 처리.
//!
//! 주요 커맨드 코드:
//! - 101 대사 시작 (MZ는 parameters[4]가 화자 이름), 401 대사 한 줄
//! - 105 스크롤 텍스트 시작, 405 스크롤 텍스트 한 줄
//! - 102 선택지 (parameters[0]이 문자열 배열)
//! - 320/324/325 이름/닉네임/프로필 변경 (parameters[1])
//! - 357 MZ 플러그인 커맨드 (parameters[3]이 인자 객체, 플러그인 추출을 켰을 때만)
//!
//! MV 플러그인 커맨드(356)는 "명령 인자1 인자2" 형태의 한 줄 문자열이라
//! 일부만 번역하면 명령 자체가 깨지므로 추출하지 않는다.

use crate::jsonspan::Node;
use crate::model::Kind;

use super::{plugins, Sink};

/// 현재 이어지고 있는 대사/스크롤 블록
struct Block {
    group: String,
    label: String,
    speaker: Option<String>,
}

pub fn extract_list(list: &Node, base: &str, label: &str, sink: &mut Sink) {
    let Some(cmds) = list.as_array() else { return };
    let mut block: Option<Block> = None;

    let new_block = |sink: &Sink, i: usize, what: &str, speaker: Option<String>| Block {
        group: sink.group_id(&format!("{base}/{i}")),
        label: format!("{label} · #{i} {what}"),
        speaker,
    };

    for (i, cmd) in cmds.iter().enumerate() {
        let code = cmd.get("code").and_then(Node::as_i64).unwrap_or(0);
        let params = cmd.get("parameters");
        let param = |n: usize| params.and_then(|p| p.at(n));
        let param_path = |n: usize| format!("{base}/{i}/parameters/{n}");

        match code {
            101 => {
                let speaker = param(4).and_then(Node::as_str).filter(|s| !s.is_empty()).map(str::to_string);
                let b = new_block(sink, i, "대사", speaker);
                sink.push(param(4), param_path(4), Kind::Speaker, &b.group, &b.label, None);
                block = Some(b);
            }
            105 => block = Some(new_block(sink, i, "스크롤", None)),
            401 | 405 => {
                // 101/105 없이 시작한 줄은 그 위치에서 새 블록으로 취급
                let b = block.get_or_insert_with(|| new_block(sink, i, "대사", None));
                let kind = if code == 401 { Kind::Dialogue } else { Kind::Scroll };
                sink.push(param(0), param_path(0), kind, &b.group, &b.label, b.speaker.as_deref());
            }
            102 => {
                block = None;
                let group = sink.group_id(&format!("{base}/{i}"));
                let group_label = format!("{label} · #{i} 선택지");
                for (j, choice) in param(0).and_then(Node::as_array).unwrap_or_default().iter().enumerate() {
                    let path = format!("{}/{j}", param_path(0));
                    sink.push(Some(choice), path, Kind::Choice, &group, &group_label, None);
                }
            }
            320 | 324 | 325 => {
                block = None;
                let (kind, what) = match code {
                    320 => (Kind::Name, "이름 변경"),
                    324 => (Kind::Nickname, "닉네임 변경"),
                    _ => (Kind::Profile, "프로필 변경"),
                };
                let group = sink.group_id(&format!("{base}/{i}"));
                sink.push(param(1), param_path(1), kind, &group, &format!("{label} · #{i} {what}"), None);
            }
            357 if sink.plugins => {
                block = None;
                let plugin = param(0).and_then(Node::as_str).unwrap_or("");
                let command = param(1).and_then(Node::as_str).unwrap_or("");
                let group = sink.group_id(&format!("{base}/{i}"));
                let group_label = format!("{label} · #{i} 플러그인 커맨드 {plugin}:{command}");
                plugins::extract_command_args(param(3), &param_path(3), &group, &group_label, sink);
            }
            _ => block = None,
        }
    }
}

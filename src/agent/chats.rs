//! The chat list's views: how saved chats are grouped and ordered (by date, most
//! recent, folder, agent), which are working now, and the archive.
//!
//! A chat nobody touched for `AUTO_ARCHIVE_DAYS` moves to the archive by itself
//! (out of the way, never deleted); one the user takes back out stays out.

use super::Chat;

/// Chats untouched this long go to the archive on their own.
pub const AUTO_ARCHIVE_DAYS: u64 = 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatView {
    /// Today, Yesterday, This week, Earlier (the default).
    #[default]
    Date,
    /// One list, newest first.
    Recent,
    /// By the folder each chat was in.
    Folder,
    /// By the agent (harness) that answered.
    Agent,
    /// Only the chat answering right now.
    Working,
    /// The archive.
    Archived,
}

impl ChatView {
    pub const ALL: [ChatView; 6] = [ChatView::Date, ChatView::Recent, ChatView::Folder, ChatView::Agent, ChatView::Working, ChatView::Archived];

    pub fn label(self) -> &'static str {
        match self {
            ChatView::Date => "By date",
            ChatView::Recent => "Most recent",
            ChatView::Folder => "By folder",
            ChatView::Agent => "By agent",
            ChatView::Working => "Working now",
            ChatView::Archived => "Archived",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            ChatView::Date => "date",
            ChatView::Recent => "recent",
            ChatView::Folder => "folder",
            ChatView::Agent => "agent",
            ChatView::Working => "working",
            ChatView::Archived => "archived",
        }
    }

    pub fn from_id(id: &str) -> ChatView {
        ChatView::ALL.into_iter().find(|v| v.id() == id).unwrap_or_default()
    }
}

/// A group heading (empty for an ungrouped list) and the indexes into `history`
/// it holds, in order.
pub type Groups = Vec<(String, Vec<usize>)>;

/// The chats `view` shows, grouped. `busy` is the chat answering now, if any;
/// `now` and `today_start` are Unix ms (today_start: local midnight).
pub fn groups(history: &[Chat], view: ChatView, busy: Option<u64>, now: u64, today_start: u64) -> Groups {
    let mut idx: Vec<usize> = (0..history.len())
        .filter(|&i| {
            let c = &history[i];
            match view {
                ChatView::Archived => c.archived,
                ChatView::Working => busy == Some(c.id),
                // A chat working right now always shows, archived or not.
                _ => !c.archived || busy == Some(c.id),
            }
        })
        .collect();
    // Working first, then newest.
    idx.sort_by_key(|&i| (busy != Some(history[i].id), std::cmp::Reverse(history[i].updated)));
    let day = 86_400_000;
    let mut out: Groups = Vec::new();
    let mut push = |key: String, i: usize| match out.iter_mut().find(|(k, _)| *k == key) {
        Some((_, v)) => v.push(i),
        None => out.push((key, vec![i])),
    };
    for i in idx {
        let c = &history[i];
        let key = match view {
            ChatView::Recent | ChatView::Working | ChatView::Archived => String::new(),
            _ if busy == Some(c.id) => "Working now".to_string(),
            ChatView::Date => {
                let _ = now;
                if c.updated >= today_start {
                    "Today"
                } else if c.updated >= today_start.saturating_sub(day) {
                    "Yesterday"
                } else if c.updated >= today_start.saturating_sub(6 * day) {
                    "This week"
                } else {
                    "Earlier"
                }
                .to_string()
            }
            ChatView::Folder => c.folder.as_deref().map(folder_name).unwrap_or_else(|| "No folder".to_string()),
            ChatView::Agent => c.agent.as_deref().map(crate::acp::display_name).unwrap_or_else(|| "Unknown agent".to_string()),
        };
        push(key, i);
    }
    out
}

/// The last part of a folder path, for a heading.
pub fn folder_name(path: &str) -> String {
    let t = path.trim_end_matches(['/', '\\']);
    t.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or(t).to_string()
}

/// Archive what nobody touched for `AUTO_ARCHIVE_DAYS` (except `keep_id`, the
/// open chat, and chats the user took back out of the archive). True when
/// something moved.
pub fn auto_archive(history: &mut [Chat], now: u64, keep_id: Option<u64>) -> bool {
    let limit = AUTO_ARCHIVE_DAYS * 86_400_000;
    let mut moved = false;
    for c in history.iter_mut() {
        if !c.archived && !c.keep && Some(c.id) != keep_id && now.saturating_sub(c.updated) > limit {
            c.archived = true;
            moved = true;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: u64, updated: u64) -> Chat {
        Chat { id, title: format!("c{id}"), renamed: false, updated, messages: Vec::new(), archived: false, keep: false, folder: None, agent: None }
    }

    const DAY: u64 = 86_400_000;

    #[test]
    fn date_groups_put_working_first() {
        let today = 100 * DAY;
        let h = vec![chat(1, today + 5), chat(2, today - DAY / 2), chat(3, today - 3 * DAY), chat(4, today - 30 * DAY), chat(5, today - 2 * DAY)];
        let g = groups(&h, ChatView::Date, Some(3), today + 10, today);
        let names: Vec<&str> = g.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["Working now", "Today", "Yesterday", "This week", "Earlier"]);
        assert_eq!(g[0].1, vec![2]);
        assert_eq!(g[3].1, vec![4]);
    }

    #[test]
    fn archive_is_its_own_view() {
        let mut h = vec![chat(1, 10 * DAY), chat(2, 1 * DAY), chat(3, 2 * DAY)];
        h[2].keep = true;
        assert!(auto_archive(&mut h, 10 * DAY + 1, None));
        assert!(!h[0].archived && h[1].archived && !h[2].archived);
        let recent = groups(&h, ChatView::Recent, None, 0, 0);
        assert_eq!(recent, vec![(String::new(), vec![0, 2])]);
        assert_eq!(groups(&h, ChatView::Archived, None, 0, 0), vec![(String::new(), vec![1])]);
        // Nothing left to move.
        assert!(!auto_archive(&mut h, 10 * DAY + 1, None));
    }

    #[test]
    fn folders_group_by_their_last_part() {
        let mut h = vec![chat(1, 3), chat(2, 2), chat(3, 1)];
        h[0].folder = Some(r"C:\work\api".into());
        h[1].folder = Some("/home/me/web/".into());
        h[2].folder = Some(r"C:\work\api".into());
        let g = groups(&h, ChatView::Folder, None, 0, 0);
        assert_eq!(g, vec![("api".to_string(), vec![0, 2]), ("web".to_string(), vec![1])]);
    }
}

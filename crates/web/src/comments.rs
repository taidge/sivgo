//! Document-level comment system for Sivgo.
//!
//! Comments are anchored to (x, y) positions on the canvas and support
//! threaded replies and resolution tracking.

/// A single comment (or reply) attached to a document.
#[derive(Debug, Clone)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub text: String,
    pub timestamp: u64,
    pub x: f64,
    pub y: f64,
    pub resolved: bool,
    pub replies: Vec<Comment>,
}

/// Manages the collection of comments for a document.
#[derive(Debug)]
pub struct CommentStore {
    comments: Vec<Comment>,
    /// Monotonic counter for generating unique IDs.
    id_counter: u64,
}

impl CommentStore {
    /// Create an empty comment store.
    pub fn new() -> Self {
        Self {
            comments: Vec::new(),
            id_counter: 0,
        }
    }

    /// Generate the next unique comment ID.
    pub fn next_id(&mut self) -> String {
        self.id_counter += 1;
        format!("id-comment-{}", self.id_counter)
    }

    /// Add a top-level comment. Returns the comment's ID.
    pub fn add(&mut self, comment: Comment) -> String {
        let id = comment.id.clone();
        self.comments.push(comment);
        id
    }

    /// Mark a comment as resolved. Returns `true` if the comment was found.
    pub fn resolve(&mut self, id: &str) -> bool {
        if let Some(c) = Self::find_mut(&mut self.comments, id) {
            c.resolved = true;
            true
        } else {
            false
        }
    }

    /// Add a reply to an existing comment. Returns `true` if the parent was found.
    pub fn reply(&mut self, parent_id: &str, reply: Comment) -> bool {
        if let Some(parent) = Self::find_mut(&mut self.comments, parent_id) {
            parent.replies.push(reply);
            true
        } else {
            false
        }
    }

    /// Delete a top-level comment by ID. Returns `true` if it was removed.
    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.comments.len();
        self.comments.retain(|c| c.id != id);
        self.comments.len() < before
    }

    /// List all top-level comments.
    pub fn list(&self) -> &[Comment] {
        &self.comments
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Recursively search for a comment by ID.
    fn find_mut<'a>(comments: &'a mut [Comment], id: &str) -> Option<&'a mut Comment> {
        // Find the index first (immutable pass), then return the mutable ref.
        let idx = Self::find_index(comments, id)?;
        Self::get_by_index(comments, idx)
    }

    /// Return the flat index path to a comment with the given ID, or `None`.
    fn find_index(comments: &[Comment], id: &str) -> Option<Vec<usize>> {
        for (i, c) in comments.iter().enumerate() {
            if c.id == id {
                return Some(vec![i]);
            }
            if let Some(mut path) = Self::find_index(&c.replies, id) {
                path.insert(0, i);
                return Some(path);
            }
        }
        None
    }

    /// Walk a pre-computed index path to obtain a mutable reference.
    fn get_by_index<'a>(comments: &'a mut [Comment], path: Vec<usize>) -> Option<&'a mut Comment> {
        let mut current = comments;
        let last = path.len() - 1;
        for (depth, &idx) in path.iter().enumerate() {
            if idx >= current.len() {
                return None;
            }
            if depth == last {
                return Some(&mut current[idx]);
            }
            current = &mut current[idx].replies;
        }
        None
    }
}

impl Default for CommentStore {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_comment(store: &mut CommentStore, author: &str, text: &str) -> Comment {
        Comment {
            id: store.next_id(),
            author: author.to_string(),
            text: text.to_string(),
            timestamp: 1000,
            x: 50.0,
            y: 100.0,
            resolved: false,
            replies: Vec::new(),
        }
    }

    #[test]
    fn add_and_list_comments() {
        let mut store = CommentStore::new();
        let c1 = make_comment(&mut store, "alice", "First comment");
        let c2 = make_comment(&mut store, "bob", "Second comment");
        store.add(c1);
        store.add(c2);

        assert_eq!(store.list().len(), 2);
        assert_eq!(store.list()[0].author, "alice");
        assert_eq!(store.list()[1].author, "bob");
    }

    #[test]
    fn resolve_comment() {
        let mut store = CommentStore::new();
        let c = make_comment(&mut store, "alice", "Needs fixing");
        let id = c.id.clone();
        store.add(c);

        assert!(!store.list()[0].resolved);
        assert!(store.resolve(&id));
        assert!(store.list()[0].resolved);
    }

    #[test]
    fn resolve_nonexistent_returns_false() {
        let mut store = CommentStore::new();
        assert!(!store.resolve("no-such-id"));
    }

    #[test]
    fn reply_to_comment() {
        let mut store = CommentStore::new();
        let c = make_comment(&mut store, "alice", "Top-level");
        let parent_id = c.id.clone();
        store.add(c);

        let reply = make_comment(&mut store, "bob", "Reply text");
        assert!(store.reply(&parent_id, reply));
        assert_eq!(store.list()[0].replies.len(), 1);
        assert_eq!(store.list()[0].replies[0].author, "bob");
    }

    #[test]
    fn reply_to_nonexistent_returns_false() {
        let mut store = CommentStore::new();
        let reply = make_comment(&mut store, "bob", "Orphan reply");
        assert!(!store.reply("ghost", reply));
    }

    #[test]
    fn delete_comment() {
        let mut store = CommentStore::new();
        let c = make_comment(&mut store, "alice", "Delete me");
        let id = c.id.clone();
        store.add(c);

        assert!(store.delete(&id));
        assert!(store.list().is_empty());
        // Deleting again is a no-op.
        assert!(!store.delete(&id));
    }

    #[test]
    fn next_id_is_unique() {
        let mut store = CommentStore::new();
        let id1 = store.next_id();
        let id2 = store.next_id();
        let id3 = store.next_id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }
}

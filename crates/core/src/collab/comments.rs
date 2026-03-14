//! Document comments and comment threads.
//!
//! Provides a simple threaded comment model where comments can be attached to
//! a position on the canvas, a specific SVG node, or left free-floating.

// ---------------------------------------------------------------------------
// Comment
// ---------------------------------------------------------------------------

/// A single comment (or reply) within a comment thread.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    /// Unique identifier for this comment.
    pub id: String,
    /// Identifier of the user who authored the comment.
    pub author_id: String,
    /// Display name of the author.
    pub author_name: String,
    /// The comment text.
    pub text: String,
    /// Unix-epoch timestamp (seconds) when the comment was created.
    pub created_at: u64,
    /// Whether the comment has been marked as resolved.
    pub resolved: bool,
    /// Optional canvas position (document coordinates) for a pin.
    pub position: Option<(f64, f64)>,
    /// Optional node id this comment is attached to.
    pub node_id: Option<String>,
    /// Replies to this comment.
    pub replies: Vec<Comment>,
}

impl Comment {
    /// Create a new top-level comment.
    pub fn new(
        id: &str,
        author_id: &str,
        author_name: &str,
        text: &str,
        created_at: u64,
    ) -> Self {
        Self {
            id: id.to_string(),
            author_id: author_id.to_string(),
            author_name: author_name.to_string(),
            text: text.to_string(),
            created_at,
            resolved: false,
            position: None,
            node_id: None,
            replies: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// CommentThread
// ---------------------------------------------------------------------------

/// A collection of top-level comments with support for replies, resolution,
/// and filtering.
#[derive(Debug, Clone)]
pub struct CommentThread {
    /// All top-level comments.
    pub comments: Vec<Comment>,
    /// Auto-incrementing counter used to generate unique ids.
    next_id: u64,
}

impl CommentThread {
    /// Create a new empty comment thread.
    pub fn new() -> Self {
        Self {
            comments: Vec::new(),
            next_id: 1,
        }
    }

    /// Generate the next unique comment id.
    fn gen_id(&mut self) -> String {
        let id = format!("comment-{}", self.next_id);
        self.next_id += 1;
        id
    }

    /// Add a new top-level comment.
    ///
    /// Returns the generated comment id.
    pub fn add_comment(
        &mut self,
        author_id: &str,
        author_name: &str,
        text: &str,
        position: Option<(f64, f64)>,
    ) -> String {
        let id = self.gen_id();
        let mut comment = Comment::new(&id, author_id, author_name, text, 0);
        comment.position = position;
        self.comments.push(comment);
        id
    }

    /// Reply to an existing comment.
    ///
    /// Returns `Some(reply_id)` if the parent comment was found, `None` otherwise.
    pub fn reply(
        &mut self,
        comment_id: &str,
        author_id: &str,
        author_name: &str,
        text: &str,
    ) -> Option<String> {
        let reply_id = self.gen_id();
        if let Some(parent) = self.find_comment_mut(comment_id) {
            let reply = Comment::new(&reply_id, author_id, author_name, text, 0);
            parent.replies.push(reply);
            Some(reply_id)
        } else {
            None
        }
    }

    /// Mark a comment as resolved.  Returns `true` if found.
    pub fn resolve(&mut self, comment_id: &str) -> bool {
        if let Some(c) = self.find_comment_mut(comment_id) {
            c.resolved = true;
            true
        } else {
            false
        }
    }

    /// Mark a comment as unresolved.  Returns `true` if found.
    pub fn unresolve(&mut self, comment_id: &str) -> bool {
        if let Some(c) = self.find_comment_mut(comment_id) {
            c.resolved = false;
            true
        } else {
            false
        }
    }

    /// Return all unresolved top-level comments.
    pub fn get_unresolved(&self) -> Vec<&Comment> {
        self.comments.iter().filter(|c| !c.resolved).collect()
    }

    /// Return all top-level comments attached to a given node.
    pub fn get_for_node(&self, node_id: &str) -> Vec<&Comment> {
        self.comments
            .iter()
            .filter(|c| c.node_id.as_deref() == Some(node_id))
            .collect()
    }

    // Recursively find a comment by id (searches top-level and replies).
    fn find_comment_mut(&mut self, id: &str) -> Option<&mut Comment> {
        for comment in &mut self.comments {
            if comment.id == id {
                return Some(comment);
            }
            if let Some(reply) = Self::find_in_replies_mut(&mut comment.replies, id) {
                return Some(reply);
            }
        }
        None
    }

    fn find_in_replies_mut<'a>(replies: &'a mut [Comment], id: &str) -> Option<&'a mut Comment> {
        for reply in replies.iter_mut() {
            if reply.id == id {
                return Some(reply);
            }
            if let Some(nested) = Self::find_in_replies_mut(&mut reply.replies, id) {
                return Some(nested);
            }
        }
        None
    }
}

impl Default for CommentThread {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_thread_empty() {
        let thread = CommentThread::new();
        assert!(thread.comments.is_empty());
    }

    #[test]
    fn test_add_comment() {
        let mut thread = CommentThread::new();
        let id = thread.add_comment("alice", "Alice", "Hello!", Some((10.0, 20.0)));
        assert_eq!(thread.comments.len(), 1);
        assert_eq!(thread.comments[0].id, id);
        assert_eq!(thread.comments[0].author_id, "alice");
        assert_eq!(thread.comments[0].text, "Hello!");
        assert_eq!(thread.comments[0].position, Some((10.0, 20.0)));
    }

    #[test]
    fn test_reply() {
        let mut thread = CommentThread::new();
        let parent_id = thread.add_comment("alice", "Alice", "Question?", None);
        let reply_id = thread.reply(&parent_id, "bob", "Bob", "Answer.");
        assert!(reply_id.is_some());
        assert_eq!(thread.comments[0].replies.len(), 1);
        assert_eq!(thread.comments[0].replies[0].text, "Answer.");
    }

    #[test]
    fn test_reply_nonexistent() {
        let mut thread = CommentThread::new();
        let result = thread.reply("no-such-id", "bob", "Bob", "Hello");
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_unresolve() {
        let mut thread = CommentThread::new();
        let id = thread.add_comment("alice", "Alice", "Fix this", None);
        assert!(!thread.comments[0].resolved);

        assert!(thread.resolve(&id));
        assert!(thread.comments[0].resolved);

        assert!(thread.unresolve(&id));
        assert!(!thread.comments[0].resolved);
    }

    #[test]
    fn test_resolve_nonexistent() {
        let mut thread = CommentThread::new();
        assert!(!thread.resolve("nope"));
        assert!(!thread.unresolve("nope"));
    }

    #[test]
    fn test_get_unresolved() {
        let mut thread = CommentThread::new();
        let id1 = thread.add_comment("alice", "Alice", "Open", None);
        let _id2 = thread.add_comment("bob", "Bob", "Also open", None);
        thread.resolve(&id1);

        let unresolved = thread.get_unresolved();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].text, "Also open");
    }

    #[test]
    fn test_get_for_node() {
        let mut thread = CommentThread::new();
        thread.add_comment("alice", "Alice", "On node A", None);
        // Manually set node_id
        thread.comments[0].node_id = Some("node-a".into());

        thread.add_comment("bob", "Bob", "On node B", None);
        thread.comments[1].node_id = Some("node-b".into());

        thread.add_comment("charlie", "Charlie", "No node", None);

        let for_a = thread.get_for_node("node-a");
        assert_eq!(for_a.len(), 1);
        assert_eq!(for_a[0].text, "On node A");

        let for_b = thread.get_for_node("node-b");
        assert_eq!(for_b.len(), 1);

        let for_c = thread.get_for_node("node-c");
        assert!(for_c.is_empty());
    }

    #[test]
    fn test_unique_ids() {
        let mut thread = CommentThread::new();
        let id1 = thread.add_comment("a", "A", "1", None);
        let id2 = thread.add_comment("b", "B", "2", None);
        let id3 = thread.add_comment("c", "C", "3", None);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_default_thread() {
        let thread = CommentThread::default();
        assert!(thread.comments.is_empty());
    }

    #[test]
    fn test_comment_new() {
        let c = Comment::new("c-1", "user-1", "User One", "Hello world", 999);
        assert_eq!(c.id, "c-1");
        assert_eq!(c.author_id, "user-1");
        assert_eq!(c.author_name, "User One");
        assert_eq!(c.text, "Hello world");
        assert_eq!(c.created_at, 999);
        assert!(!c.resolved);
        assert!(c.position.is_none());
        assert!(c.node_id.is_none());
        assert!(c.replies.is_empty());
    }

    #[test]
    fn test_nested_reply() {
        let mut thread = CommentThread::new();
        let top_id = thread.add_comment("alice", "Alice", "Top", None);
        let reply_id = thread.reply(&top_id, "bob", "Bob", "Reply").unwrap();
        let nested_id = thread.reply(&reply_id, "charlie", "Charlie", "Nested");
        assert!(nested_id.is_some());
        assert_eq!(thread.comments[0].replies[0].replies.len(), 1);
        assert_eq!(
            thread.comments[0].replies[0].replies[0].text,
            "Nested"
        );
    }
}

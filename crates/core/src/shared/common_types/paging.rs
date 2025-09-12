use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PagingContext {
    pub limit: u32,
    pub offset: u32,
}

impl PagingContext {
    /// Creates a new PagingContext with the specified limit and offset.
    ///
    /// # Arguments
    /// * `limit` - The maximum number of items to return
    /// * `offset` - The number of items to skip
    ///
    /// # Returns
    /// * `Self` - A new PagingContext instance
    pub fn new(limit: u32, offset: u32) -> Self {
        PagingContext { limit, offset }
    }

    /// Creates the next pagination context based on the current result length.
    ///
    /// Returns None if there are no more results (result_length is 0).
    ///
    /// # Arguments
    /// * `result_length` - The number of items in the current result set
    ///
    /// # Returns
    /// * `Some(Self)` - The next pagination context if more results may exist
    /// * `None` - If no more results are available
    pub fn next(&self, result_length: usize) -> Option<Self> {
        if result_length == 0 {
            return None;
        }

        Some(PagingContext { limit: self.limit, offset: self.offset + self.limit })
    }

    /// Creates the previous pagination context.
    ///
    /// Returns None if already at the beginning (offset is less than or equal to limit).
    ///
    /// # Returns
    /// * `Some(Self)` - The previous pagination context if not at the beginning
    /// * `None` - If already at the beginning
    pub fn previous(&self) -> Option<Self> {
        let offset = if self.offset > self.limit {
            self.offset - self.limit
        } else {
            return None;
        };

        Some(PagingContext { limit: self.limit, offset })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PagingResult<TResult: Serialize> {
    pub items: Vec<TResult>,
    pub next: Option<PagingContext>,
    pub previous: Option<PagingContext>,
}

impl<TResult: Serialize> PagingResult<TResult> {
    /// Creates a new PagingResult with items and pagination contexts.
    ///
    /// # Arguments
    /// * `items` - The result items for this page
    /// * `next` - Optional context for the next page
    /// * `previous` - Optional context for the previous page
    ///
    /// # Returns
    /// * `Self` - A new PagingResult instance
    pub fn new(
        items: Vec<TResult>,
        next: Option<PagingContext>,
        previous: Option<PagingContext>,
    ) -> Self {
        PagingResult { items, next, previous }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PagingQuery {
    pub limit: u32,
    pub offset: u32,
}

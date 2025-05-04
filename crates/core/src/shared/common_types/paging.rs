use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PagingContext {
    pub limit: u32,
    pub offset: u32,
}

impl PagingContext {
    pub fn new(limit: u32, offset: u32) -> Self {
        PagingContext { limit, offset }
    }

    pub fn next(&self, result_length: usize) -> Option<Self> {
        if result_length == 0 {
            return None;
        }

        Some(PagingContext { limit: self.limit, offset: self.offset + self.limit })
    }

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
    next: Option<PagingContext>,
    previous: Option<PagingContext>,
}

impl<TResult: Serialize> PagingResult<TResult> {
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

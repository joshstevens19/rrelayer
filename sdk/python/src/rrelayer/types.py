from pydantic import BaseModel, ConfigDict


class PagingContext(BaseModel):
    limit: int
    offset: int

    model_config = ConfigDict(extra="forbid")


defaultPagingContext = PagingContext(limit=100, offset=0)

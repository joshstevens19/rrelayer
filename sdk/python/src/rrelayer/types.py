from enum import Enum
from typing import List, Optional

from pydantic import BaseModel, ConfigDict


class TransactionSpeed(Enum):
    SLOW = "SLOW"
    MEDIUM = "MEDIUM"
    FAST = "FAST"
    SUPER = "SUPER"


class PagingContext(BaseModel):
    limit: int
    offset: int

    model_config = ConfigDict(extra="forbid")


defaultPagingContext = PagingContext(limit=100, offset=0)


class TransactionToSend(BaseModel):
    to: str
    value: Optional[str | int] = None
    data: Optional[str] = None
    speed: Optional[TransactionSpeed] = None
    blobs: Optional[List[str]] = None
    externalId: Optional[str] = None

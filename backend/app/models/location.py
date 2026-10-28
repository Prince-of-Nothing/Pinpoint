"""
Location update model.

Privacy by design (data minimisation): only the *latest* point per user is
retained (one row per owner, upserted on each update). Historical movement
trails are an explicitly deferred, separately consented feature — see
docs/architecture/movement-trails-and-search-corridors.md.
"""

from sqlalchemy import Column, DateTime, Float, ForeignKey, Integer
from sqlalchemy.orm import relationship
from sqlalchemy.sql import func

from app.db import Base


class LocationUpdate(Base):
    """Latest known location for a user."""

    __tablename__ = "location_updates"

    id = Column(Integer, primary_key=True, index=True)
    owner_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), unique=True, nullable=False)
    latitude = Column(Float, nullable=False)
    longitude = Column(Float, nullable=False)
    accuracy = Column(Float, nullable=True)  # metres, device-reported
    battery_level = Column(Integer, nullable=True)  # 0-100
    recorded_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False, index=True)

    owner = relationship("User", back_populates="location_update")

    def __repr__(self):
        return f"<LocationUpdate(owner={self.owner_id}, recorded_at={self.recorded_at})>"

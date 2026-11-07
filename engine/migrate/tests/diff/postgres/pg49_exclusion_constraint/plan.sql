alter table bookings add constraint bookings_room_during_excl exclude using gist (room with =, during with &&) where ((room > 0));

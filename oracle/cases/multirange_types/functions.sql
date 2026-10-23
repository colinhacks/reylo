select id, lower(seats) as first, upper(seats) as last, isempty(seats) as empty, range_merge(seats) as merged, seats @> :n as holds from bookings

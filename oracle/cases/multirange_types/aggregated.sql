select range_agg(r) as all_seats, range_intersect_agg(seats) as common from bookings, unnest(seats) r

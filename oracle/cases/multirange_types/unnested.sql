select b.id, r from bookings b, unnest(b.seats) r

insert into rooms (id, during) values (:id::int, tstzrange(:start::timestamptz, :finish::timestamptz))

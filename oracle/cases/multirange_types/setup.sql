-- Multirange columns, each seeded with no range, one, and two with a gap between them.
create table bookings (id int primary key, nights datemultirange, seats int4multirange not null, prices nummultirange, spans int8multirange[]);

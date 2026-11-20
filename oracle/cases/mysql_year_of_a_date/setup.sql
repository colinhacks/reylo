-- A YEAR column holds 0 and 1901 to 2155, but MySQL types every value of a YEAR's protocol type
-- as one: `year(d)` of a date up to 9999-12-31 among them. Only a table's own column is bounded.
create table days (id int primary key, d date not null, dt datetime, y year not null);
insert into days values (1, '9999-12-31', '9999-12-31 23:59:59', 2155), (2, '2026-01-01', null, 0);

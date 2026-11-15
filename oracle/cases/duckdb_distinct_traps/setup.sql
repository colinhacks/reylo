-- What DuckDB keeps distinct that it finds equal: text under a NOCASE collation, intervals list_distinct keeps apart, and lists whose union casts both arms to one type.
create table c (id integer primary key, s varchar collate nocase, i interval, x decimal(38, 10), y decimal(38, 0));
insert into c values (1, 'a', interval 1 day, 1.0000000001, 5), (2, 'A', interval 24 hours, 1.0000000002, 5), (3, 'b', interval 1 month, null, null);

-- A case-sensitive regular expression a CHECK holds a string to is a pattern, the I-Regexp of exactly the values it finds a match in: `~` matches anywhere unless anchored, and its `.` matches a line break. A case-insensitive one, and one with no exact I-Regexp, says nothing.
create domain hex as text check (value ~ '^[0-9a-f]+$');
create table codes (
  id int primary key,
  code varchar(10) check (code ~ '^[A-Z]{3}[0-9]{2,4}$'),
  slug text check (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
  mail text check (mail ~ '@'),
  line text check (line ~ '^a.c$'),
  either text check (either ~ 'x$|^y'),
  lazy text check (lazy ~ '^a+?(?:b|c){1,2}$'),
  bracket text check (bracket ~ '^[^-\]a]+$'),
  digits text check (digits ~ '^\d+$'),
  word text check (word ~ '\mfoo'),
  folded text check (folded ~* '^a'),
  embedded text check (embedded ~ '(?i)^a'),
  h hex
);
insert into codes values
  (1, 'ABC12', 'a-b-c', 'x@y', E'a\nc', E'ax', 'abc', E'b\nc', '123', 'foo', 'A', 'A', '0af'),
  (2, 'XYZ1234', 'z', '@', 'abc', E'y\nz', 'aaab', 'xyz', '0', 'a foo', 'a', 'a', 'f'),
  (3, null, null, null, null, E'q\nx', null, null, null, null, null, null, null);

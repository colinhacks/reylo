-- JSON_TABLE gives a column of a character type the connection's character set and collation,
-- with the coercibility of a table's column: mysql2 connects with utf8mb4_unicode_ci, and a
-- database MySQL 8 makes is utf8mb4_0900_ai_ci, so comparing such a column with a table's failed
-- with "Illegal mix of collations" (1267). The compiled statement gives each the database's
-- character set and collation in its definition; a type that names its own, and a number, are
-- left as written. The column keeps a column's coercibility, so a table's column of another
-- collation decides as MySQL lets it: a _bin one wins (`code`), and one of the connection's
-- collation, which compared before, is refused (`legacy`) until the JSON_TABLE names it.
create table users (id int primary key, name varchar(40) not null, tag varchar(20), code varchar(20) collate utf8mb4_bin, legacy varchar(20) collate utf8mb4_unicode_ci);
create table posts (id int primary key, title text not null);

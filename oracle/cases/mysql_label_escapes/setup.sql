-- information_schema writes an ENUM's or a SET's labels with a backslash before a line feed (`\n`),
-- a carriage return (`\r`), a NUL (`\0`) and a backslash, and a quote doubled.
create table labelled (id int primary key, e enum('a\nb', 'cr\rx', 'b\\s', 'it''s', 'q') not null, s set('x\ny', 'z') not null);
insert into labelled values (1, 'a\nb', 'x\ny'), (2, 'cr\rx', ''), (3, 'b\\s', 'z'), (4, 'it''s', 'x\ny,z');

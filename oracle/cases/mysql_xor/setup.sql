-- MySQL ranks XOR between AND and OR, so a key equality or a null test beside one is no conjunct
-- of the WHERE: `id = ? and x = 1 xor y = 2` is `(id = ? and x = 1) xor (y = 2)`.
create table k (id int primary key, x int not null, y int);
create table o (id int primary key, note varchar(10));

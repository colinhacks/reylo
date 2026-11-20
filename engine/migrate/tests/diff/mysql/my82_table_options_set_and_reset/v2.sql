create table linked (id int);
create table placed (id int);
create table a (id int) engine = myisam;
create table b (id int) engine = myisam;
create table merged (id int) engine = merge union = (a, b) insert_method = first;
create table keyed (id int, key ix (id) key_block_size = 8) engine = myisam;
create table texts (id int primary key, t text, fulltext key ft (t));
create table hashed (id int) partition by key algorithm = 1 (id) partitions 2;
create table invoices (id int primary key auto_increment, total int) auto_increment = 1000;

create table linked (id int) connection = 'mysql://reader@remote/app/t';
create table placed (id int) tablespace = innodb_system;
create table a (id int) engine = myisam;
create table b (id int) engine = myisam;
create table merged (id int) engine = merge union = (a) insert_method = last;
create table keyed (id int, key ix (id) key_block_size = 4) engine = myisam;
create table texts (id int primary key, t text, fulltext key ft (t) with parser ngram);

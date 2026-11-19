create table p (id int primary key); create table c (id int primary key, seq int not null auto_increment, constraint fk foreign key (seq) references p (id));

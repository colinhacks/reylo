alter table users alter column age set data type bigint;
create table posts (id bigint, user_id bigint, foreign key (user_id) references users (id));

create table users (id int not null, constraint users_id primary key (id));
create algorithm = merge sql security definer view some_view as (select * from users) with local check option;

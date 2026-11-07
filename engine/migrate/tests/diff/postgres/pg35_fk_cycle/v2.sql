create table things (id integer primary key);
create table authors (id integer primary key, favourite_book integer);
create table books (id integer primary key, author integer not null references authors);
alter table authors add constraint authors_favourite_book_fkey foreign key (favourite_book) references books;

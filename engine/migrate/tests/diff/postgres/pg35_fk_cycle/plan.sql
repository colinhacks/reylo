create table authors (
  id integer not null,
  favourite_book integer,
  constraint authors_pkey primary key (id)
);
create table books (
  id integer not null,
  author integer not null,
  constraint books_pkey primary key (id)
);
alter table authors add constraint authors_favourite_book_fkey foreign key (favourite_book) references books(id);
alter table books add constraint books_author_fkey foreign key (author) references authors(id);

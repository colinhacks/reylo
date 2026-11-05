insert into users (id, email, name) select g, 'u' || g || '@example.com', 'User ' || g from generate_series(1, 20) g;
insert into posts (id, author_id, title, score) select g, (g % 20) + 1, 'Post ' || g, g % 7 from generate_series(1, 50000) g;
analyze;

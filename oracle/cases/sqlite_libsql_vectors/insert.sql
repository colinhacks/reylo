insert into docs (title, emb) values (:title, vector(:v)) returning id, emb

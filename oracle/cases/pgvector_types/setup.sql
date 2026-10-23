-- pgvector's types, as Supabase and Neon ship them: vector, halfvec and sparsevec columns with and
-- without a dimension, arrays of them, the distance operators against a parameter, and vectors
-- inside JSON documents, where the extension has no cast to JSON and each is written as its text.
create extension vector;
create table items (
  id int primary key,
  embedding vector(3) not null,
  draft vector,
  half halfvec(3),
  sparse sparsevec(5),
  history vector(3)[],
  halves halfvec[] not null default '{}'
);

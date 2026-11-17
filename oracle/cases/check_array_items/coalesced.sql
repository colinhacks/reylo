-- An empty array in place of a NULL one keeps what the CHECK says of the column's elements, as an
-- empty literal, an empty ARRAY[] and a cast literal; a column with no such CHECK, and a fallback
-- that spells NULL, keep them nullable.
select
  coalesce(scores, '{}') as literal,
  coalesce(scores, array[]::int[]) as built,
  coalesce(scores, '{}'::int[]) as cast,
  coalesce(l, '{}') as domain,
  coalesce(loose, '{}') as unchecked,
  coalesce(scores, '{NULL}') as null_fallback
from posts

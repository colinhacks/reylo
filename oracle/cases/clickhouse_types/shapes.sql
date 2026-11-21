SELECT
  groupArray(id) AS ids, groupArray((id, s)) AS pairs, groupArray(CAST((id, maybe), 'Tuple(id UInt64, maybe Nullable(String))')) AS rows,
  arrayMap(x -> x * 2, [1, 2, 3]) AS doubled, map('a', [toNullable(1)]) AS nested, (1, ('a', [NULL, 1])) AS inner,
  sumMap(counts) AS summed, arrayJoin([1, 2]) AS each, CAST(maybe, 'Variant(String, UInt8)') AS either
FROM every GROUP BY maybe ORDER BY each

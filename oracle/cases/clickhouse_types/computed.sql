SELECT
  u8 + 1 AS sum8, u32 * 2 AS product, u8 / 2 AS quotient, intDiv(i32, 3) AS whole, d64 * 2 AS scaled, d64 / 3 AS divided,
  toString(id) AS text, length(tags) AS n, tags[1] AS first_tag, counts['k'] AS k, pair.1 AS left, named.label AS label,
  toDate(at) AS d, toStartOfHour(at3) AS hour, at3 + INTERVAL 1 DAY AS tomorrow, dateDiff('second', at, at_zone) AS apart,
  upper(s) AS loud, concat(s, packed) AS joined, toTypeName(spot) AS type_name, CAST(role, 'String') AS role_text, role = 'admin' AS is_admin
FROM every ORDER BY id

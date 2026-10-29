select enum_range(null::mood) as moods, unnest(enum_range(null::mood)) as each

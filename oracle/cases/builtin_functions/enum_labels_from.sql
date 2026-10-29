select label from unnest(enum_range('ok'::mood, null)) as label

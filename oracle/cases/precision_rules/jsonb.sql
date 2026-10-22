select to_jsonb(m) ? 'id' as h, to_jsonb(m) ?| array['x'] as a, to_jsonb(m) ?& array['id'] as l, to_jsonb(m) #- '{id}' as d from member m

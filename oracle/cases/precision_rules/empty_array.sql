select coalesce(array_agg(id), array[]::int[]) as ids, array[]::text[] as e from member

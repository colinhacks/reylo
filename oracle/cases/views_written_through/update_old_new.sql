update published set published_at = null where id = :id returning old.published_at as was, new.published_at as now

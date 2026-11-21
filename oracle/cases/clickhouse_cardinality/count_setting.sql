SELECT count() AS n FROM users WHERE age > {than:UInt8} SETTINGS empty_result_for_aggregation_by_empty_set = 1

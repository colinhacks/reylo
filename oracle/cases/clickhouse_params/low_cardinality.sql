SELECT id, concat(plan, {suffix:String}) AS label, plan = {plan:String} AS on_plan FROM users WHERE created >= toDateTime({since:String})

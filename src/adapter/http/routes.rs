use super::dto::{
    agent_state_routes, board_state_routes, company_state_routes, contract_state_routes,
    query_routes, runtime_routes, work_state_routes, RouteDto, EVENTS_ROUTE, METHOD_GET,
};

pub(crate) fn all_routes() -> Vec<RouteDto> {
    let mut routes = Vec::new();
    routes.extend(query_routes());
    routes.extend(board_state_routes());
    routes.extend(company_state_routes());
    routes.extend(contract_state_routes());
    routes.extend(work_state_routes());
    routes.extend(agent_state_routes());
    routes.extend(runtime_routes());
    routes.push(RouteDto {
        method: METHOD_GET,
        path: EVENTS_ROUTE,
        handler: "after_commit_sse_stream",
    });
    routes
}

#[cfg(test)]
mod tests {
    use crate::adapter::http::dto::{
        AGENT_PAUSE_ROUTE, AGENT_RESUME_ROUTE, COMPANIES_ROUTE, CONTRACTS_COLLECTION_ROUTE,
        EVENTS_ROUTE, RUN_DETAIL_ROUTE, WORK_INTENTS_ROUTE, WORK_UPDATE_ROUTE,
    };

    use super::all_routes;

    #[test]
    fn canonical_transport_surface_is_present() {
        let routes = all_routes();
        assert!(routes.iter().any(|route| route.path == "/api/board"));
        assert!(routes.iter().any(|route| route.path == COMPANIES_ROUTE));
        assert!(routes
            .iter()
            .any(|route| route.path == CONTRACTS_COLLECTION_ROUTE));
        assert!(routes.iter().any(|route| route.path == WORK_INTENTS_ROUTE));
        assert!(routes.iter().any(|route| route.path == WORK_UPDATE_ROUTE));
        assert!(routes.iter().any(|route| route.path == AGENT_PAUSE_ROUTE));
        assert!(routes.iter().any(|route| route.path == AGENT_RESUME_ROUTE));
        assert!(routes.iter().any(|route| route.path == EVENTS_ROUTE));
        assert!(routes.iter().any(|route| route.path == RUN_DETAIL_ROUTE));
        assert!(
            routes
                .iter()
                .filter(|route| route.path == WORK_INTENTS_ROUTE)
                .count()
                == 1
        );
    }
}

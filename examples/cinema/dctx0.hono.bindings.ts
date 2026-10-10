import { defineHonoBindings } from "@runatron/plugin-dctx0"

export default defineHonoBindings({
  version: 3,
  surfaces: ["HomeCinema.MovieNight.Catalog.FilmCatalogApi", "HomeCinema.MovieNight.Community.FilmCommunityApi", "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi"],
  root: "src/http/app.ts",
  factory: "createHttpApp",
  entrypoint: { root: "src/app/app.ts", export: "app" },
  operations: { root: "dctx0/hono/operations", include: "*.ts" },
  families: { cinemaApi: { authentication: "none", requestTransport: "hono", failureMapping: "declared-status", responseEnvelope: "json" } }
})

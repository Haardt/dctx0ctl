import { defineDctx0Bindings } from "@runatron/plugin-dctx0"

export default defineDctx0Bindings({
  version: 3,
  declarations: {
    "HomeCinema.MovieNight.AddComment": { type: { root: "src/community/contracts.ts", export: "AddComment" }, decoder: { root: "src/community/contracts.ts", export: "decodeAddComment" } },
    "HomeCinema.MovieNight.CastVote": { type: { root: "src/screeningplanning/contracts.ts", export: "CastVote" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodeCastVote" } },
    "HomeCinema.MovieNight.ChangeAvailability": { type: { root: "src/catalog/contracts.ts", export: "ChangeAvailability" }, decoder: { root: "src/catalog/contracts.ts", export: "decodeChangeAvailability" } },
    "HomeCinema.MovieNight.Comment": { type: { root: "src/community/contracts.ts", export: "Comment" }, decoder: { root: "src/community/contracts.ts", export: "decodeComment" } },
    "HomeCinema.MovieNight.CommentAdded": { type: { root: "src/community/contracts.ts", export: "CommentAdded" }, decoder: { root: "src/community/contracts.ts", export: "decodeCommentAdded" } },
    "HomeCinema.MovieNight.CommentId": { type: { root: "src/community/primitives.ts", export: "CommentId" }, decoder: { root: "src/community/contracts.ts", export: "decodeCommentId" } },
    "HomeCinema.MovieNight.CommentNotFound": { type: { root: "src/movie-night/contracts.ts", export: "CommentNotFound" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeCommentNotFound" } },
    "HomeCinema.MovieNight.CommentOverview": { type: { root: "src/movie-night/contracts.ts", export: "CommentOverview" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeCommentOverview" } },
    "HomeCinema.MovieNight.CommentView": { type: { root: "src/movie-night/contracts.ts", export: "CommentView" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeCommentView" } },
    "HomeCinema.MovieNight.Film": { type: { root: "src/catalog/contracts.ts", export: "Film" }, decoder: { root: "src/catalog/contracts.ts", export: "decodeFilm" } },
    "HomeCinema.MovieNight.FilmAvailability": { type: { root: "src/movie-night/contracts.ts", export: "FilmAvailability" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeFilmAvailability" } },
    "HomeCinema.MovieNight.FilmAvailabilityChanged": { type: { root: "src/catalog/contracts.ts", export: "FilmAvailabilityChanged" }, decoder: { root: "src/catalog/contracts.ts", export: "decodeFilmAvailabilityChanged" } },
    "HomeCinema.MovieNight.FilmDetails": { type: { root: "src/movie-night/contracts.ts", export: "FilmDetails" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeFilmDetails" } },
    "HomeCinema.MovieNight.FilmId": { type: { root: "src/catalog/primitives.ts", export: "FilmId" }, decoder: { root: "src/catalog/contracts.ts", export: "decodeFilmId" } },
    "HomeCinema.MovieNight.FilmNotFound": { type: { root: "src/movie-night/contracts.ts", export: "FilmNotFound" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeFilmNotFound" } },
    "HomeCinema.MovieNight.FilmOverview": { type: { root: "src/movie-night/contracts.ts", export: "FilmOverview" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeFilmOverview" } },
    "HomeCinema.MovieNight.FilmRated": { type: { root: "src/community/contracts.ts", export: "FilmRated" }, decoder: { root: "src/community/contracts.ts", export: "decodeFilmRated" } },
    "HomeCinema.MovieNight.FilmSaved": { type: { root: "src/catalog/contracts.ts", export: "FilmSaved" }, decoder: { root: "src/catalog/contracts.ts", export: "decodeFilmSaved" } },
    "HomeCinema.MovieNight.FilmSearch": { type: { root: "src/catalog/contracts.ts", export: "FilmSearch" }, decoder: { root: "src/catalog/contracts.ts", export: "decodeFilmSearch" } },
    "HomeCinema.MovieNight.FilmSort": { type: { root: "src/movie-night/contracts.ts", export: "FilmSort" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeFilmSort" } },
    "HomeCinema.MovieNight.FilmSummary": { type: { root: "src/movie-night/contracts.ts", export: "FilmSummary" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeFilmSummary" } },
    "HomeCinema.MovieNight.FilmVote": { type: { root: "src/screeningplanning/contracts.ts", export: "FilmVote" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodeFilmVote" } },
    "HomeCinema.MovieNight.FilmVoteResult": { type: { root: "src/movie-night/contracts.ts", export: "FilmVoteResult" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeFilmVoteResult" } },
    "HomeCinema.MovieNight.Genre": { type: { root: "src/movie-night/contracts.ts", export: "Genre" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeGenre" } },
    "HomeCinema.MovieNight.MemberId": { type: { root: "src/community/primitives.ts", export: "MemberId" }, decoder: { root: "src/community/contracts.ts", export: "decodeMemberId" } },
    "HomeCinema.MovieNight.OpenVoting": { type: { root: "src/screeningplanning/contracts.ts", export: "OpenVoting" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodeOpenVoting" } },
    "HomeCinema.MovieNight.PageLimit": { type: { root: "src/movie-night/contracts.ts", export: "PageLimit" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodePageLimit" } },
    "HomeCinema.MovieNight.PageOffset": { type: { root: "src/movie-night/contracts.ts", export: "PageOffset" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodePageOffset" } },
    "HomeCinema.MovieNight.PersistenceUnavailable": { type: { root: "src/movie-night/contracts.ts", export: "PersistenceUnavailable" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodePersistenceUnavailable" } },
    "HomeCinema.MovieNight.PollId": { type: { root: "src/screeningplanning/primitives.ts", export: "PollId" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodePollId" } },
    "HomeCinema.MovieNight.PollNotFound": { type: { root: "src/movie-night/contracts.ts", export: "PollNotFound" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodePollNotFound" } },
    "HomeCinema.MovieNight.PollStatus": { type: { root: "src/movie-night/contracts.ts", export: "PollStatus" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodePollStatus" } },
    "HomeCinema.MovieNight.RateFilm": { type: { root: "src/community/contracts.ts", export: "RateFilm" }, decoder: { root: "src/community/contracts.ts", export: "decodeRateFilm" } },
    "HomeCinema.MovieNight.Rating": { type: { root: "src/community/contracts.ts", export: "Rating" }, decoder: { root: "src/community/contracts.ts", export: "decodeRating" } },
    "HomeCinema.MovieNight.RatingSummary": { type: { root: "src/movie-night/contracts.ts", export: "RatingSummary" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeRatingSummary" } },
    "HomeCinema.MovieNight.ReleaseYear": { type: { root: "src/movie-night/contracts.ts", export: "ReleaseYear" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeReleaseYear" } },
    "HomeCinema.MovieNight.RuntimeMinutes": { type: { root: "src/movie-night/contracts.ts", export: "RuntimeMinutes" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeRuntimeMinutes" } },
    "HomeCinema.MovieNight.SaveFilm": { type: { root: "src/catalog/contracts.ts", export: "SaveFilm" }, decoder: { root: "src/catalog/contracts.ts", export: "decodeSaveFilm" } },
    "HomeCinema.MovieNight.ScreeningPoll": { type: { root: "src/screeningplanning/contracts.ts", export: "ScreeningPoll" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodeScreeningPoll" } },
    "HomeCinema.MovieNight.StarRating": { type: { root: "src/movie-night/contracts.ts", export: "StarRating" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeStarRating" } },
    "HomeCinema.MovieNight.VoteCast": { type: { root: "src/screeningplanning/contracts.ts", export: "VoteCast" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodeVoteCast" } },
    "HomeCinema.MovieNight.VotingClosed": { type: { root: "src/movie-night/contracts.ts", export: "VotingClosed" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeVotingClosed" } },
    "HomeCinema.MovieNight.VotingCompleted": { type: { root: "src/screeningplanning/contracts.ts", export: "VotingCompleted" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodeVotingCompleted" } },
    "HomeCinema.MovieNight.VotingOpened": { type: { root: "src/screeningplanning/contracts.ts", export: "VotingOpened" }, decoder: { root: "src/screeningplanning/contracts.ts", export: "decodeVotingOpened" } },
    "HomeCinema.MovieNight.VotingOverview": { type: { root: "src/movie-night/contracts.ts", export: "VotingOverview" }, decoder: { root: "src/movie-night/contracts.ts", export: "decodeVotingOverview" } }
  },
  modules: {
    "HomeCinema.MovieNight.Catalog": "src/catalog",
    "HomeCinema.MovieNight.Community": "src/community",
    "HomeCinema.MovieNight.ScreeningPlanning": "src/screeningplanning"
  },
  surfaces: {
    "HomeCinema.MovieNight.Catalog.FilmCatalogApi": {
      operations: {
        "HomeCinema.MovieNight.Catalog.FilmCatalogApi.changeFilmAvailability": { root: "src/catalog/use-cases/changeFilmAvailability.ts", export: "changeFilmAvailability" },
        "HomeCinema.MovieNight.Catalog.FilmCatalogApi.getFilm": { root: "src/catalog/use-cases/getFilm.ts", export: "getFilm" },
        "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm": { root: "src/catalog/use-cases/saveFilm.ts", export: "saveFilm" },
        "HomeCinema.MovieNight.Catalog.FilmCatalogApi.searchFilms": { root: "src/catalog/use-cases/searchFilms.ts", export: "searchFilms" }
      }
    },
    "HomeCinema.MovieNight.Community.FilmCommunityApi": {
      operations: {
        "HomeCinema.MovieNight.Community.FilmCommunityApi.addComment": { root: "src/community/use-cases/addComment.ts", export: "addComment" },
        "HomeCinema.MovieNight.Community.FilmCommunityApi.getFilmRating": { root: "src/community/use-cases/getFilmRating.ts", export: "getFilmRating" },
        "HomeCinema.MovieNight.Community.FilmCommunityApi.listComments": { root: "src/community/use-cases/listComments.ts", export: "listComments" },
        "HomeCinema.MovieNight.Community.FilmCommunityApi.rateFilm": { root: "src/community/use-cases/rateFilm.ts", export: "rateFilm" }
      }
    },
    "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi": {
      operations: {
        "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.castVote": { root: "src/screeningplanning/use-cases/castVote.ts", export: "castVote" },
        "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.closeVoting": { root: "src/screeningplanning/use-cases/closeVoting.ts", export: "closeVoting" },
        "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.getCurrentVoting": { root: "src/screeningplanning/use-cases/getCurrentVoting.ts", export: "getCurrentVoting" },
        "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.openVoting": { root: "src/screeningplanning/use-cases/openVoting.ts", export: "openVoting" }
      }
    },
    "HomeCinema.MovieNight.ScreeningPlanning.VotingRules": {
      operations: {
        "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner": { root: "src/screeningplanning/candidateOrderWinner.ts", export: "candidateOrderWinner" }
      }
    }
  },
  ports: {
    "HomeCinema.MovieNight.Catalog.FilmCatalogStore": { roots: ["src/catalog/adapters/filmCatalogStore.ts"], conformance: { local: { root: "src/catalog/conformance/filmCatalogStore.ts", export: "createFilmCatalogStoreConformanceFixture" } } },
    "HomeCinema.MovieNight.Community.CommentStore": { roots: ["src/community/adapters/commentStore.ts"], conformance: { local: { root: "src/community/conformance/commentStore.ts", export: "createCommentStoreConformanceFixture" } } },
    "HomeCinema.MovieNight.Community.FilmPresence": { roots: ["src/community/adapters/filmPresence.ts"], conformance: { local: { root: "src/community/conformance/filmPresence.ts", export: "createFilmPresenceConformanceFixture" } } },
    "HomeCinema.MovieNight.Community.RatingStore": { roots: ["src/community/adapters/ratingStore.ts"], conformance: { local: { root: "src/community/conformance/ratingStore.ts", export: "createRatingStoreConformanceFixture" } } },
    "HomeCinema.MovieNight.ScreeningPlanning.VotingStore": { roots: ["src/screeningplanning/adapters/votingStore.ts"], conformance: { local: { root: "src/screeningplanning/conformance/votingStore.ts", export: "createVotingStoreConformanceFixture" } } }
  },
  application: { root: "src/app/application.ts", factory: "createApplication" },
  platformPackages: ["hono"]
})

// Code generated from the central application bundle; DO NOT EDIT.
package contracttests

import (
	"cinema/domain"
	"fmt"
)

func (fixture *cinemaFixture) production(invocation appInvocation, arguments []any) (any, error) {
	switch invocation.Subject {
	case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.changeFilmAvailability":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation0(arg0)
	case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.getFilm":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		return fixture.operation1(arg0)
	case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation2(arg0)
	case "HomeCinema.MovieNight.Catalog.FilmCatalogApi.searchFilms":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation3(arg0)
	case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.changeAvailability":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation4(arg0)
	case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.find":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		return fixture.operation5(arg0)
	case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.save":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation6(arg0)
	case "HomeCinema.MovieNight.Catalog.FilmCatalogStore.search":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation7(arg0)
	case "HomeCinema.MovieNight.Community.CommentStore.append":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation8(arg0)
	case "HomeCinema.MovieNight.Community.CommentStore.listByFilm":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		return fixture.operation9(arg0)
	case "HomeCinema.MovieNight.Community.FilmCommunityApi.addComment":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation10(arg0)
	case "HomeCinema.MovieNight.Community.FilmCommunityApi.getFilmRating":
		if len(arguments) != 2 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		arg1, ok := arguments[1].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 1 differs from generated string")
		}
		return fixture.operation11(arg0, arg1)
	case "HomeCinema.MovieNight.Community.FilmCommunityApi.listComments":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		return fixture.operation12(arg0)
	case "HomeCinema.MovieNight.Community.FilmCommunityApi.rateFilm":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation13(arg0)
	case "HomeCinema.MovieNight.Community.FilmPresence.exists":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		return fixture.operation14(arg0)
	case "HomeCinema.MovieNight.Community.RatingStore.summary":
		if len(arguments) != 2 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		arg1, ok := arguments[1].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 1 differs from generated string")
		}
		return fixture.operation15(arg0, arg1)
	case "HomeCinema.MovieNight.Community.RatingStore.upsert":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation16(arg0)
	case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.castVote":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation17(arg0)
	case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.closeVoting":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		return fixture.operation18(arg0)
	case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.getCurrentVoting":
		if len(arguments) != 0 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		return fixture.operation19()
	case "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.openVoting":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation20(arg0)
	case "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner":
		if len(arguments) != 2 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].([]any)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated []any")
		}
		arg1, ok := arguments[1].([]any)
		if !ok {
			return nil, fmt.Errorf("Native argument 1 differs from generated []any")
		}
		return fixture.operation21(arg0, arg1)
	case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.cast":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation22(arg0)
	case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.close":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(string)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated string")
		}
		return fixture.operation23(arg0)
	case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.current":
		if len(arguments) != 0 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		return fixture.operation24()
	case "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.open":
		if len(arguments) != 1 {
			return nil, fmt.Errorf("Incorrect generated arity")
		}
		arg0, ok := arguments[0].(domain.Record)
		if !ok {
			return nil, fmt.Errorf("Native argument 0 differs from generated domain.Record")
		}
		return fixture.operation25(arg0)
	default:
		return nil, fmt.Errorf("Operation has no generated application binding: %s", invocation.Subject)
	}
}
func (fixture *cinemaFixture) operation0(arg0 domain.Record) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("changeFilmAvailability", []any{arg0})
	}
	return fixture.service.Invoke("changeFilmAvailability", []any{arg0})
}
func (fixture *cinemaFixture) operation1(arg0 string) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("getFilm", []any{arg0})
	}
	return fixture.service.Invoke("getFilm", []any{arg0})
}
func (fixture *cinemaFixture) operation2(arg0 domain.Record) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("saveFilm", []any{arg0})
	}
	return fixture.service.Invoke("saveFilm", []any{arg0})
}
func (fixture *cinemaFixture) operation3(arg0 domain.Record) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("searchFilms", []any{arg0})
	}
	return fixture.service.Invoke("searchFilms", []any{arg0})
}
func (fixture *cinemaFixture) operation4(arg0 domain.Record) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Catalog.FilmCatalogStore.changeAvailability", []any{arg0})
}
func (fixture *cinemaFixture) operation5(arg0 string) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Catalog.FilmCatalogStore.find", []any{arg0})
}
func (fixture *cinemaFixture) operation6(arg0 domain.Record) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Catalog.FilmCatalogStore.save", []any{arg0})
}
func (fixture *cinemaFixture) operation7(arg0 domain.Record) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Catalog.FilmCatalogStore.search", []any{arg0})
}
func (fixture *cinemaFixture) operation8(arg0 domain.Record) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Community.CommentStore.append", []any{arg0})
}
func (fixture *cinemaFixture) operation9(arg0 string) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Community.CommentStore.listByFilm", []any{arg0})
}
func (fixture *cinemaFixture) operation10(arg0 domain.Record) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("addComment", []any{arg0})
	}
	return fixture.service.Invoke("addComment", []any{arg0})
}
func (fixture *cinemaFixture) operation11(arg0 string, arg1 string) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("getFilmRating", []any{arg0, arg1})
	}
	return fixture.service.Invoke("getFilmRating", []any{arg0, arg1})
}
func (fixture *cinemaFixture) operation12(arg0 string) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("listComments", []any{arg0})
	}
	return fixture.service.Invoke("listComments", []any{arg0})
}
func (fixture *cinemaFixture) operation13(arg0 domain.Record) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("rateFilm", []any{arg0})
	}
	return fixture.service.Invoke("rateFilm", []any{arg0})
}
func (fixture *cinemaFixture) operation14(arg0 string) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Community.FilmPresence.exists", []any{arg0})
}
func (fixture *cinemaFixture) operation15(arg0 string, arg1 string) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Community.RatingStore.summary", []any{arg0, arg1})
}
func (fixture *cinemaFixture) operation16(arg0 domain.Record) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.Community.RatingStore.upsert", []any{arg0})
}
func (fixture *cinemaFixture) operation17(arg0 domain.Record) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("castVote", []any{arg0})
	}
	return fixture.service.Invoke("castVote", []any{arg0})
}
func (fixture *cinemaFixture) operation18(arg0 string) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("closeVoting", []any{arg0})
	}
	return fixture.service.Invoke("closeVoting", []any{arg0})
}
func (fixture *cinemaFixture) operation19() (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("getCurrentVoting", []any{})
	}
	return fixture.service.Invoke("getCurrentVoting", []any{})
}
func (fixture *cinemaFixture) operation20(arg0 domain.Record) (any, error) {
	if fixture.server != nil {
		return fixture.invokeHTTP("openVoting", []any{arg0})
	}
	return fixture.service.Invoke("openVoting", []any{arg0})
}
func (fixture *cinemaFixture) operation21(arg0 []any, arg1 []any) (any, error) {
	return domain.CandidateOrderWinner(arg0, arg1), nil
}
func (fixture *cinemaFixture) operation22(arg0 domain.Record) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.cast", []any{arg0})
}
func (fixture *cinemaFixture) operation23(arg0 string) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.close", []any{arg0})
}
func (fixture *cinemaFixture) operation24() (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.current", []any{})
}
func (fixture *cinemaFixture) operation25(arg0 domain.Record) (any, error) {
	return fixture.store.Invoke("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.open", []any{arg0})
}

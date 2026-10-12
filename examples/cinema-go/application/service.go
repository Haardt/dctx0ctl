// Package application composes ordinary Go interfaces; collaborators may be real or substituted.
package application

import "cinema/domain"

type Ports interface {
	Invoke(operation string, arguments []any) (any, error)
}
type Event struct {
	Name    string
	Payload domain.Record
}
type Service struct {
	ports   Ports
	publish func(Event) error
}

func New(ports Ports, publish func(Event) error) *Service { return &Service{ports, publish} }
func (s *Service) emit(name string, payload domain.Record) error {
	if err := domain.Validate(name, payload); err != nil {
		return err
	}
	return s.publish(Event{name, payload})
}

var surfaces = map[string]string{
	"searchFilms": "Catalog.FilmCatalogStore.search", "getFilm": "Catalog.FilmCatalogStore.find", "saveFilm": "Catalog.FilmCatalogStore.save", "changeFilmAvailability": "Catalog.FilmCatalogStore.changeAvailability",
	"listComments": "Community.CommentStore.listByFilm", "addComment": "Community.CommentStore.append", "getFilmRating": "Community.RatingStore.summary", "rateFilm": "Community.RatingStore.upsert",
	"openVoting": "ScreeningPlanning.VotingStore.open", "getCurrentVoting": "ScreeningPlanning.VotingStore.current", "castVote": "ScreeningPlanning.VotingStore.cast", "closeVoting": "ScreeningPlanning.VotingStore.close",
}

func (s *Service) Invoke(operation string, args []any) (any, error) {
	port, ok := surfaces[operation]
	if !ok {
		return nil, domain.NewFailure("PersistenceUnavailable", domain.Record{"reason": "Unknown operation"})
	}
	inputs := map[string][]string{
		"searchFilms": {"FilmSearch"}, "getFilm": {"FilmId"}, "saveFilm": {"SaveFilm"}, "changeFilmAvailability": {"ChangeAvailability"},
		"listComments": {"FilmId"}, "addComment": {"AddComment"}, "getFilmRating": {"FilmId", "MemberId"}, "rateFilm": {"RateFilm"},
		"openVoting": {"OpenVoting"}, "getCurrentVoting": {}, "castVote": {"CastVote"}, "closeVoting": {"PollId"},
	}
	if len(args) != len(inputs[operation]) {
		return nil, &domain.ContractError{Message: "Incorrect operation arity"}
	}
	for i, contract := range inputs[operation] {
		if err := domain.Validate(contract, args[i]); err != nil {
			return nil, err
		}
	}
	var request domain.Record
	if len(args) > 0 {
		request, _ = args[0].(domain.Record)
	}
	if operation == "listComments" || operation == "addComment" || operation == "getFilmRating" || operation == "rateFilm" {
		id := args[0]
		if request != nil {
			id = request["filmId"]
		}
		present, err := s.ports.Invoke("Community.FilmPresence.exists", []any{id})
		if err != nil {
			return nil, err
		}
		if present == nil {
			return nil, domain.NewFailure("FilmNotFound", domain.Record{"filmId": id})
		}
	}
	result, err := s.ports.Invoke(port, args)
	if err != nil {
		return nil, err
	}
	if operation == "getFilm" && result == nil {
		return nil, domain.NewFailure("FilmNotFound", domain.Record{"filmId": args[0]})
	}
	var name string
	var payload domain.Record
	switch operation {
	case "saveFilm":
		name = "FilmSaved"
		payload = domain.Record{"filmId": request["id"]}
	case "changeFilmAvailability":
		name = "FilmAvailabilityChanged"
		payload = request
	case "addComment":
		name = "CommentAdded"
		payload = domain.Record{"commentId": result.(domain.Record)["id"], "filmId": request["filmId"]}
	case "rateFilm":
		name = "FilmRated"
		payload = request
	case "openVoting":
		name = "VotingOpened"
		payload = domain.Record{"pollId": request["pollId"], "scheduledFor": request["scheduledFor"]}
	case "castVote":
		name = "VoteCast"
		payload = request
	case "closeVoting":
		name = "VotingCompleted"
		payload = domain.Record{"pollId": args[0], "winningFilmId": result.(domain.Record)["winningFilmId"]}
	}
	if name != "" {
		if err := s.emit(name, payload); err != nil {
			return nil, err
		}
	}
	return result, nil
}

package resolve

import "sort"

// similar returns the candidate closest to name within an edit budget
// that grows with the name's length (short names would otherwise match
// almost anything), preferring the alphabetically first on ties, or "".
func similar(name string, candidates []string) string {
	budget := editBudget(len([]rune(name)))
	if budget == 0 {
		return ""
	}
	best, bestDist := "", budget+1
	sorted := append([]string(nil), candidates...)
	sort.Strings(sorted)
	for _, c := range sorted {
		if c == name {
			continue
		}
		if d := editDistance(name, c); d < bestDist {
			best, bestDist = c, d
		}
	}
	return best
}

// editBudget is how many edits may separate a typo from its intended name.
func editBudget(n int) int {
	switch {
	case n <= 2:
		return 0
	case n <= 4:
		return 1
	}
	return 2
}

// editDistance is the optimal string alignment distance in runes: like
// Levenshtein, but a transposition of adjacent runes (fiels → files)
// counts as one edit.
func editDistance(a, b string) int {
	ra, rb := []rune(a), []rune(b)
	d := make([][]int, len(ra)+1)
	for i := range d {
		d[i] = make([]int, len(rb)+1)
		d[i][0] = i
	}
	for j := range d[0] {
		d[0][j] = j
	}
	for i := 1; i <= len(ra); i++ {
		for j := 1; j <= len(rb); j++ {
			cost := 1
			if ra[i-1] == rb[j-1] {
				cost = 0
			}
			d[i][j] = min(d[i-1][j]+1, d[i][j-1]+1, d[i-1][j-1]+cost)
			if i > 1 && j > 1 && ra[i-1] == rb[j-2] && ra[i-2] == rb[j-1] {
				d[i][j] = min(d[i][j], d[i-2][j-2]+1)
			}
		}
	}
	return d[len(ra)][len(rb)]
}

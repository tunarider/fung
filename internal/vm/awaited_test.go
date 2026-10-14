package vm

import (
	"bytes"
	"testing"

	"github.com/tunarider/fung/internal/value"
)

// A helper that awaits every task it spawns (pmap, fan_in) must never see
// a failure reported as unawaited. The task is marked awaited before it
// can run, so even a task that fails at once finds a waiter (CI under
// -race once saw it finish first and report "task failed").
func TestSpawnAwaitedCountsTheWaiterBeforeTheTaskRuns(t *testing.T) {
	for i := range 500 {
		seen := make(chan int32, 1)
		probe := &value.Native{Name: "probe", Fn: func(ctx value.Context, _ []value.Value) (value.Value, error) {
			seen <- ctx.(*Task).handle.waiters.Load()
			return value.Nil{}, nil
		}}
		rt := &Runtime{Stdout: &bytes.Buffer{}, Stderr: &bytes.Buffer{}}
		if _, err := spawnAwaitedFn(rt.newTask(rt.root()), []value.Value{probe}); err != nil {
			t.Fatal(err)
		}
		if w := <-seen; w < 1 {
			t.Fatalf("run %d: the task ran with %d waiters; a failure here would be reported as unawaited", i, w)
		}
		rt.Wait()
	}
}

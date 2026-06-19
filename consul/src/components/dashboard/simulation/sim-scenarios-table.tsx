import {
  simDataShellVariant,
  simTableRowClassName,
} from '@/components/dashboard/simulation/constants';
import type { SimScenario } from '@/data/simulation';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';

const statusVariant = {
  ready: 'outline',
  running: 'default',
  passed: 'secondary',
  failed: 'destructive',
} as const;

type SimScenariosTableProps = {
  scenarios: SimScenario[];
};

export function SimScenariosTable({ scenarios }: SimScenariosTableProps) {
  return (
    <Card variant={simDataShellVariant}>
      <CardHeader>
        <CardDescription>Scenarios</CardDescription>
        <CardTitle className="text-lg font-semibold">Isaac Lab tasks</CardTitle>
      </CardHeader>
      <CardContent className="px-0 pb-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>World</TableHead>
              <TableHead className="hidden md:table-cell">Description</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="hidden sm:table-cell">Last run</TableHead>
              <TableHead className="w-24 text-right">Run</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {scenarios.map((scenario) => (
              <TableRow key={scenario.id} className={simTableRowClassName}>
                <TableCell className="font-mono text-xs">{scenario.name}</TableCell>
                <TableCell className="font-mono text-xs">{scenario.world}</TableCell>
                <TableCell className="hidden max-w-xs truncate text-muted-foreground md:table-cell">
                  {scenario.description}
                </TableCell>
                <TableCell>
                  <Badge variant={statusVariant[scenario.status]}>{scenario.status}</Badge>
                </TableCell>
                <TableCell className="hidden font-mono text-xs sm:table-cell">
                  {scenario.lastRun}
                </TableCell>
                <TableCell className="text-right">
                  <Button size="sm" variant="outline" disabled>
                    Run
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

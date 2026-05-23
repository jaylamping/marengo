import type { ReactNode } from 'react';

import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { cn } from '@/lib/utils';

type DashboardCardShellProps = {
  description: string;
  title: string;
  titleClassName?: string;
  action?: ReactNode;
  content?: ReactNode;
  footerPrimary: ReactNode;
  footerSecondary: ReactNode;
  className?: string;
};

export function DashboardCardShell({
  description,
  title,
  titleClassName,
  action,
  content,
  footerPrimary,
  footerSecondary,
  className,
}: DashboardCardShellProps) {
  return (
    <Card className={cn('@container/card', className)}>
      <CardHeader>
        <CardDescription>{description}</CardDescription>
        <CardTitle className={cn('text-lg font-semibold', titleClassName)}>
          {title}
        </CardTitle>
        {action ? <CardAction>{action}</CardAction> : null}
      </CardHeader>
      {content ? <CardContent className="px-6 pb-0">{content}</CardContent> : null}
      <CardFooter className="flex-col items-start gap-1.5 text-sm">
        <div className="line-clamp-1 font-medium">{footerPrimary}</div>
        <div className="text-muted-foreground">{footerSecondary}</div>
      </CardFooter>
    </Card>
  );
}
